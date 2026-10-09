use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, PoisonError};

use otelo_indexed_storage::query::SpanGroupingField;
use otelo_indexed_storage::{AttributeValue, Attributes, SpanKind, SpanStatus};
use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Value};

use super::span_groups::{GroupingValuesAsJson, list_grouping_columns_as_json};
use super::traces::compile_span_query;
use super::{SPAN_ALIASES, WhereClause};
use crate::Reader;
use crate::rollup::{HOUR_NS, MINUTE_NS};
use crate::span_summary::{SpanSummary, SpanSummaryKeyId};

// Past this many, a query reads the spans of every key, and not through an IN list of them.
const MAX_LISTED_SPAN_SUMMARY_KEYS: usize = 2_000;

pub(super) struct DecodedSpanSummaryKey {
    pub service: Arc<str>,
    pub name: Arc<str>,
    kind: SpanKind,
    status: SpanStatus,
    resource: Arc<Attributes>,
    pub attributes: Attributes,
    unsummarized_keys: HashSet<Arc<str>>,
}

type DecodedKeyList = Vec<(SpanSummaryKeyId, Arc<DecodedSpanSummaryKey>)>;

// A key changes only when retention deletes it, so the daemon keeps them decoded, reads the new
// ones, and reads them all again once one is gone.
#[derive(Default)]
pub struct SpanSummaryKeyCache(Mutex<DecodedSpanSummaryKeys>);

#[derive(Clone, Default)]
struct DecodedSpanSummaryKeys {
    newest_key_id: i64,
    key_count: usize,
    keys: Arc<DecodedKeyList>,
}

struct GroupOfSpanSummaryKey {
    service: Arc<str>,
    name: Arc<str>,
    resource: Arc<Attributes>,
}

pub(super) struct SpanSummaryRow {
    pub key: Arc<DecodedSpanSummaryKey>,
    pub grouping_values: GroupingValuesAsJson,
    pub start_at: i64,
    pub summary: SpanSummary,
}

impl SpanSummaryRow {
    pub fn failed(&self) -> bool {
        self.key.status.is_error()
    }
}

enum KeyMatch {
    Matches,
    Misses,
    CannotTell,
}

enum FieldOfKey<'a> {
    Absent,
    Known(Cow<'a, AttributeValue>),
    LeftOut,
    NotSummarized,
}

struct SummaryPart {
    table: &'static str,
    since: i64,
    until: i64,
}

impl KeyMatch {
    const fn from_bool(matches: bool) -> Self {
        if matches { Self::Matches } else { Self::Misses }
    }
}

impl DecodedSpanSummaryKey {
    fn read_field(&self, field: &Field) -> FieldOfKey<'_> {
        let text = |text: &str| FieldOfKey::Known(Cow::Owned(AttributeValue::String(text.into())));
        match field {
            Field::Builtin(BuiltinField::Service) => text(&self.service),
            Field::Builtin(BuiltinField::Name) => text(&self.name),
            Field::Builtin(BuiltinField::Kind) => text(self.kind.name()),
            Field::Builtin(BuiltinField::Status) => text(self.status.name()),
            Field::Builtin(BuiltinField::Error) => {
                FieldOfKey::Known(Cow::Owned(AttributeValue::Bool(self.status.is_error())))
            }
            Field::Builtin(_) => FieldOfKey::NotSummarized,
            Field::Attribute(key) => match self.attributes.get(key) {
                Some(value) => FieldOfKey::Known(Cow::Borrowed(value)),
                None if self.unsummarized_keys.contains(key.as_str()) => FieldOfKey::LeftOut,
                None => FieldOfKey::Absent,
            },
            Field::Resource(key) => self.resource.get(key).map_or(FieldOfKey::Absent, |value| {
                FieldOfKey::Known(Cow::Borrowed(value))
            }),
        }
    }

    fn read_grouping_values(&self, by: &[SpanGroupingField]) -> Option<GroupingValuesAsJson> {
        by.iter()
            .map(|field| {
                let field = match field {
                    SpanGroupingField::Service => Field::Builtin(BuiltinField::Service),
                    SpanGroupingField::Name => Field::Builtin(BuiltinField::Name),
                    SpanGroupingField::Attribute(key) => Field::Attribute(key.clone()),
                    SpanGroupingField::Resource(key) => Field::Resource(key.clone()),
                };
                match self.read_field(&field) {
                    FieldOfKey::Absent => Some(None),
                    FieldOfKey::Known(value) => Some(Some(value.to_string())),
                    FieldOfKey::LeftOut | FieldOfKey::NotSummarized => None,
                }
            })
            .collect()
    }

    fn match_expression(&self, expression: &Expression) -> KeyMatch {
        match expression {
            Expression::And(expressions) => {
                let mut key_match = KeyMatch::Matches;
                for expression in expressions {
                    match self.match_expression(expression) {
                        KeyMatch::Misses => return KeyMatch::Misses,
                        KeyMatch::CannotTell => key_match = KeyMatch::CannotTell,
                        KeyMatch::Matches => {}
                    }
                }
                key_match
            }
            Expression::Or(expressions) => {
                let mut key_match = KeyMatch::Misses;
                for expression in expressions {
                    match self.match_expression(expression) {
                        KeyMatch::Matches => return KeyMatch::Matches,
                        KeyMatch::CannotTell => key_match = KeyMatch::CannotTell,
                        KeyMatch::Misses => {}
                    }
                }
                key_match
            }
            Expression::Not(expression) => match self.match_expression(expression) {
                KeyMatch::Matches => KeyMatch::Misses,
                KeyMatch::Misses => KeyMatch::Matches,
                KeyMatch::CannotTell => KeyMatch::CannotTell,
            },
            Expression::Has(field) => match self.read_field(field) {
                FieldOfKey::Absent => KeyMatch::Misses,
                FieldOfKey::Known(_) | FieldOfKey::LeftOut => KeyMatch::Matches,
                FieldOfKey::NotSummarized => KeyMatch::CannotTell,
            },
            Expression::Compare {
                field,
                operator,
                value,
            } => self.match_field(field, *operator == Operator::Ne, |field_value| {
                compare_values(field_value, *operator, value)
            }),
            Expression::In { field, values } => self.match_field(field, false, |field_value| {
                values
                    .iter()
                    .any(|value| compare_values(field_value, Operator::Eq, value))
            }),
            Expression::Contains { field, text } => {
                self.match_field(field, false, |field_value| match field_value {
                    AttributeValue::String(field_text) => field_text.contains(text.as_str()),
                    _ => false,
                })
            }
        }
    }

    fn match_field(
        &self,
        field: &Field,
        absent_matches: bool,
        compare: impl Fn(&AttributeValue) -> bool,
    ) -> KeyMatch {
        match self.read_field(field) {
            FieldOfKey::Absent => KeyMatch::from_bool(absent_matches),
            FieldOfKey::Known(value) => KeyMatch::from_bool(compare(&value)),
            FieldOfKey::LeftOut | FieldOfKey::NotSummarized => KeyMatch::CannotTell,
        }
    }

    fn match_query(&self, query: &Query) -> KeyMatch {
        query
            .expression
            .as_ref()
            .map_or(KeyMatch::Matches, |expression| {
                self.match_expression(expression)
            })
    }
}

fn compare_values(field_value: &AttributeValue, operator: Operator, value: &Value) -> bool {
    let ordering = match (field_value, value) {
        (AttributeValue::String(text), Value::String(expected)) => {
            Some(text.as_str().cmp(expected))
        }
        (AttributeValue::Bool(flag), Value::Bool(expected)) => Some(flag.cmp(expected)),
        (field_value, value) => number_of_attribute(field_value)
            .zip(number_of_value(value))
            .and_then(|(number, expected)| number.partial_cmp(&expected)),
    };
    let Some(ordering) = ordering else {
        return operator == Operator::Ne;
    };
    match operator {
        Operator::Eq => ordering.is_eq(),
        Operator::Ne => ordering.is_ne(),
        Operator::Lt => ordering.is_lt(),
        Operator::Le => ordering.is_le(),
        Operator::Gt => ordering.is_gt(),
        Operator::Ge => ordering.is_ge(),
    }
}

#[expect(clippy::cast_precision_loss, reason = "a comparison")]
fn number_of_attribute(value: &AttributeValue) -> Option<f64> {
    match value {
        AttributeValue::Int(integer) => Some(*integer as f64),
        AttributeValue::Double(double) => Some(*double),
        AttributeValue::String(text) => text.parse().ok(),
        _ => None,
    }
}

#[expect(clippy::cast_precision_loss, reason = "a comparison")]
fn number_of_value(value: &Value) -> Option<f64> {
    match value {
        Value::Int(integer) | Value::Duration(integer) => Some(*integer as f64),
        Value::Float(float) => Some(*float),
        Value::String(text) => text.parse().ok(),
        Value::Bool(_) => None,
    }
}

// Whole hours come from the hour summaries when the step is a whole number of hours, whole
// minutes from the minute summaries, and the spans of the minutes the range cuts from the spans.
fn split_range_into_summary_parts(start_at: i64, end_at: i64, step_ns: i64) -> Vec<SummaryPart> {
    let first_minute_at = round_up_to_step(start_at, MINUTE_NS);
    let end_minute_at = end_at.div_euclid(MINUTE_NS) * MINUTE_NS;
    if first_minute_at >= end_minute_at {
        return Vec::new();
    }
    let minutes = |since, until| SummaryPart {
        table: "span_minute_summaries",
        since,
        until,
    };
    let first_hour_at = round_up_to_step(first_minute_at, HOUR_NS);
    let end_hour_at = end_minute_at.div_euclid(HOUR_NS) * HOUR_NS;
    if step_ns % HOUR_NS != 0 || first_hour_at >= end_hour_at {
        return vec![minutes(first_minute_at, end_minute_at)];
    }
    vec![
        minutes(first_minute_at, first_hour_at),
        SummaryPart {
            table: "span_hour_summaries",
            since: first_hour_at,
            until: end_hour_at,
        },
        minutes(end_hour_at, end_minute_at),
    ]
}

const fn round_up_to_step(at: i64, step_ns: i64) -> i64 {
    -(-at).div_euclid(step_ns) * step_ns
}

fn find_cut_minutes(start_at: i64, end_at: i64) -> Vec<(i64, i64)> {
    let first_minute_at = round_up_to_step(start_at, MINUTE_NS);
    let end_minute_at = end_at.div_euclid(MINUTE_NS) * MINUTE_NS;
    if first_minute_at >= end_minute_at {
        return vec![(start_at, end_at)];
    }
    [(start_at, first_minute_at), (end_minute_at, end_at)]
        .into_iter()
        .filter(|(since, until)| since < until)
        .collect()
}

impl Reader {
    fn read_decoded_span_summary_keys(&self) -> anyhow::Result<Arc<DecodedKeyList>> {
        let (newest_key_id, key_count): (i64, i64) = self.connection().query_row(
            "SELECT coalesce(max(id), 0), count(*) FROM span_summary_keys",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let key_count = usize::try_from(key_count)?;
        let cached = self
            .span_summary_key_cache()
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if cached.newest_key_id == newest_key_id && cached.key_count == key_count {
            return Ok(cached.keys);
        }
        let mut keys = if cached.newest_key_id <= newest_key_id {
            (*cached.keys).clone()
        } else {
            Vec::new()
        };
        let after_key_id = keys.last().map_or(0, |(key_id, _)| key_id.0);
        keys.extend(self.decode_span_summary_keys(after_key_id)?);
        if keys.len() != key_count {
            keys = self.decode_span_summary_keys(0)?;
        }
        let keys = Arc::new(keys);
        *self
            .span_summary_key_cache()
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = DecodedSpanSummaryKeys {
            newest_key_id,
            key_count,
            keys: Arc::clone(&keys),
        };
        Ok(keys)
    }

    fn decode_span_summary_keys(&self, after_key_id: i64) -> anyhow::Result<DecodedKeyList> {
        let mut where_clause = WhereClause::new();
        where_clause.push_param(":after_key_id", after_key_id);
        let key_names: HashMap<i64, Arc<str>> = self
            .collect_rows(
                "SELECT id, key FROM attribute_keys",
                &WhereClause::new(),
                |row| Ok((row.get(0)?, Arc::from(row.get::<_, String>(1)?))),
            )?
            .into_iter()
            .collect();
        let groups = self.read_groups_of_span_summary_keys(&where_clause)?;
        let rows: Vec<(SpanSummaryKeyId, i64, i32, i32, String, String)> = self.collect_rows(
            "SELECT span_summary_key.id, span_summary_key.record_group_id, span_summary_key.kind,
                    span_summary_key.status_code, span_summary_key.summarized_attributes,
                    span_summary_key.unsummarized_attribute_key_ids
             FROM span_summary_keys span_summary_key
             WHERE span_summary_key.id > :after_key_id
             ORDER BY span_summary_key.id",
            &where_clause,
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        let mut summarized_by_key = Vec::with_capacity(rows.len());
        for (_, _, _, _, summarized, _) in &rows {
            summarized_by_key.push(serde_json::from_str::<HashMap<i64, AttributeValue>>(
                summarized,
            )?);
        }
        let mut keys = Vec::with_capacity(rows.len());
        for ((key_id, group_id, kind, status_code, _, unsummarized), summarized) in
            rows.into_iter().zip(summarized_by_key)
        {
            let Some(group) = groups.get(&group_id) else {
                continue;
            };
            let mut attributes = Attributes::new();
            for (attribute_key_id, value) in summarized {
                if let Some(key) = key_names.get(&attribute_key_id) {
                    attributes.insert(&**key, value);
                }
            }
            let unsummarized_key_ids: Vec<i64> = serde_json::from_str(&unsummarized)?;
            keys.push((
                key_id,
                Arc::new(DecodedSpanSummaryKey {
                    service: Arc::clone(&group.service),
                    name: Arc::clone(&group.name),
                    kind: SpanKind::from_number(kind),
                    status: SpanStatus::from_number(status_code),
                    resource: Arc::clone(&group.resource),
                    attributes,
                    unsummarized_keys: unsummarized_key_ids
                        .iter()
                        .filter_map(|attribute_key_id| key_names.get(attribute_key_id).cloned())
                        .collect(),
                }),
            ));
        }
        Ok(keys)
    }

    fn read_groups_of_span_summary_keys(
        &self,
        where_clause: &WhereClause,
    ) -> anyhow::Result<HashMap<i64, GroupOfSpanSummaryKey>> {
        let mut resources: HashMap<i64, (Arc<str>, Arc<Attributes>)> = HashMap::new();
        let mut groups = HashMap::new();
        self.scan_rows(
            "SELECT record_group.id, record_group.name, resource.id, resource.service,
                    resource.attributes
             FROM record_groups record_group
             JOIN resources resource ON resource.id = record_group.resource_id
             WHERE record_group.id IN (SELECT span_summary_key.record_group_id
                                       FROM span_summary_keys span_summary_key
                                       WHERE span_summary_key.id > :after_key_id)",
            where_clause,
            |row| {
                let resource_id: i64 = row.get(2)?;
                if let Entry::Vacant(entry) = resources.entry(resource_id) {
                    let attributes: String = row.get(4)?;
                    entry.insert((
                        Arc::from(row.get::<_, String>(3)?),
                        Arc::new(serde_json::from_str(&attributes)?),
                    ));
                }
                let (service, resource) = &resources[&resource_id];
                groups.insert(
                    row.get(0)?,
                    GroupOfSpanSummaryKey {
                        service: Arc::clone(service),
                        name: Arc::from(row.get::<_, String>(1)?),
                        resource: Arc::clone(resource),
                    },
                );
                Ok(ControlFlow::Continue(()))
            },
        )?;
        Ok(groups)
    }

    // None when the query matches too many keys whose summaries cannot answer it, so the
    // caller reads every span of the range instead.
    pub(super) fn read_span_summary_rows(
        &self,
        query: &Query,
        by: &[SpanGroupingField],
        step_ns: i64,
    ) -> anyhow::Result<Option<Vec<SpanSummaryRow>>> {
        let keys = self.read_decoded_span_summary_keys()?;
        let mut summarized_keys = HashMap::new();
        let mut keys_of_spans = HashMap::new();
        for (key_id, key) in keys.iter() {
            let grouping_values = match key.match_query(query) {
                KeyMatch::Misses => continue,
                KeyMatch::CannotTell => None,
                KeyMatch::Matches => key.read_grouping_values(by),
            };
            match grouping_values {
                Some(grouping_values) => {
                    summarized_keys.insert(*key_id, (Arc::clone(key), grouping_values));
                }
                None => {
                    keys_of_spans.insert(*key_id, Arc::clone(key));
                }
            }
        }
        if keys_of_spans.len() > MAX_LISTED_SPAN_SUMMARY_KEYS {
            return Ok(None);
        }
        let range = self.range();
        let mut rows = Vec::new();
        for part in split_range_into_summary_parts(range.start_at(), range.end_at(), step_ns) {
            self.read_summary_part(&part, &summarized_keys, &mut rows)?;
        }
        let summarized_keys: HashMap<_, _> = summarized_keys
            .into_iter()
            .map(|(key_id, (key, _))| (key_id, key))
            .collect();
        for (since, until) in find_cut_minutes(range.start_at(), range.end_at()) {
            self.read_span_rows(query, by, &summarized_keys, Some((since, until)), &mut rows)?;
        }
        self.read_span_rows(query, by, &keys_of_spans, None, &mut rows)?;
        Ok(Some(rows))
    }

    fn read_summary_part(
        &self,
        part: &SummaryPart,
        keys: &HashMap<SpanSummaryKeyId, (Arc<DecodedSpanSummaryKey>, GroupingValuesAsJson)>,
        rows: &mut Vec<SpanSummaryRow>,
    ) -> anyhow::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let key_ids: Vec<i64> = keys.keys().map(|key_id| key_id.0).collect();
        let mut where_clause = WhereClause::new();
        where_clause.push_param(":key_ids", serde_json::to_string(&key_ids)?);
        where_clause.push_param(":since", part.since);
        where_clause.push_param(":until", part.until);
        self.scan_rows(
            &format!(
                "SELECT summary.span_summary_key_id, summary.start_at, summary.span_count,
                        summary.duration_sum_ns, summary.duration_min_ns, summary.duration_max_ns,
                        summary.duration_histogram
                 FROM {} summary
                 WHERE summary.span_summary_key_id IN (SELECT value FROM json_each(:key_ids))
                   AND summary.start_at >= :since AND summary.start_at < :until",
                part.table
            ),
            &where_clause,
            |row| {
                let key_id: SpanSummaryKeyId = row.get(0)?;
                if let Some((key, grouping_values)) = keys.get(&key_id) {
                    rows.push(SpanSummaryRow {
                        key: Arc::clone(key),
                        grouping_values: grouping_values.clone(),
                        start_at: row.get(1)?,
                        summary: SpanSummary::read_from_row(row, 2)?,
                    });
                }
                Ok(ControlFlow::Continue(()))
            },
        )
    }

    fn read_span_rows(
        &self,
        query: &Query,
        by: &[SpanGroupingField],
        keys: &HashMap<SpanSummaryKeyId, Arc<DecodedSpanSummaryKey>>,
        window: Option<(i64, i64)>,
        rows: &mut Vec<SpanSummaryRow>,
    ) -> anyhow::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let (mut where_clause, _) = compile_span_query(self, query)?;
        let key_ids: Vec<i64> = keys.keys().map(|key_id| key_id.0).collect();
        where_clause.push_condition_with_param(
            "span.span_summary_key_id IN (SELECT value FROM json_each(:span_summary_key_ids))",
            ":span_summary_key_ids",
            serde_json::to_string(&key_ids)?,
        );
        if let Some((since, until)) = window {
            where_clause.push_condition_with_param(
                "span.started_at >= :window_since",
                ":window_since",
                since,
            );
            where_clause.push_condition_with_param(
                "span.started_at < :window_until",
                ":window_until",
                until,
            );
        }
        let sql = format!(
            "SELECT span.span_summary_key_id, span.started_at, span.duration_ns{}
             FROM {}
             WHERE {}",
            list_grouping_columns_as_json(self, by)?,
            SPAN_ALIASES.encoded_records_sql("spans"),
            where_clause.sql()
        );
        self.scan_rows(&sql, &where_clause, |row| {
            let key_id: SpanSummaryKeyId = row.get(0)?;
            if let Some(key) = keys.get(&key_id) {
                rows.push(SpanSummaryRow {
                    key: Arc::clone(key),
                    grouping_values: (0..by.len())
                        .map(|index| row.get(3 + index))
                        .collect::<rusqlite::Result<_>>()?,
                    start_at: row.get(1)?,
                    summary: SpanSummary::of_span(row.get(2)?),
                });
            }
            Ok(ControlFlow::Continue(()))
        })
    }

    // The keys of the spans a query can match, or None when too many can.
    pub(super) fn find_span_summary_keys_of_query(
        &self,
        query: &Query,
    ) -> anyhow::Result<Option<Vec<i64>>> {
        let keys = self.read_decoded_span_summary_keys()?;
        let key_ids: Vec<i64> = keys
            .iter()
            .filter(|(_, key)| !matches!(key.match_query(query), KeyMatch::Misses))
            .map(|(key_id, _)| key_id.0)
            .collect();
        Ok((key_ids.len() <= MAX_LISTED_SPAN_SUMMARY_KEYS).then_some(key_ids))
    }
}
