use std::collections::HashMap;
use std::ops::ControlFlow;

use otelo_indexed_storage::query::{SpanGroup, SpanGroupingField, SpanGroups};
use otelo_indexed_storage::{AttributeValue, Attributes, SpanStatus};
use otelo_query::Query;

use super::span_stats::{SpanTally, fill_span_buckets};
use super::traces::compile_span_query;
use super::{WhereClause, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::indexes::attribute_json_path;

type GroupingValuesAsJson = Vec<Option<String>>;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SpanRowid(i64);

#[derive(Clone, Copy)]
struct NewestSpan {
    started_at: i64,
    rowid: SpanRowid,
}

#[derive(Default)]
struct GroupTally {
    spans: SpanTally,
    newest_span: Option<NewestSpan>,
}

impl GroupTally {
    fn add_span(&mut self, started_at: i64, duration_ns: i64, failed: bool, rowid: SpanRowid) {
        self.spans.add_span(duration_ns, failed);
        if self
            .newest_span
            .is_none_or(|newest_span| started_at >= newest_span.started_at)
        {
            self.newest_span = Some(NewestSpan { started_at, rowid });
        }
    }
}

struct SpanNameAndAttributes {
    name: String,
    attributes: Attributes,
}

// The JSON of a value keeps a bool apart from 1, which json_extract turns it into.
fn grouping_column_as_json(field: &SpanGroupingField) -> String {
    match field {
        SpanGroupingField::Service => "json_quote(resource.service)".to_owned(),
        SpanGroupingField::Name => "json_quote(span.name)".to_owned(),
        SpanGroupingField::Attribute(key) => {
            format!("span.attributes -> {}", attribute_json_path(key))
        }
        SpanGroupingField::Resource(key) => {
            format!("resource.attributes -> {}", attribute_json_path(key))
        }
    }
}

impl Reader {
    fn read_span_names_and_attributes(
        &self,
        rowids: impl Iterator<Item = SpanRowid>,
    ) -> anyhow::Result<HashMap<SpanRowid, SpanNameAndAttributes>> {
        let rowid_list = rowids
            .map(|rowid| rowid.0.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        if rowid_list.is_empty() {
            return Ok(HashMap::new());
        }
        let sql =
            format!("SELECT rowid, name, attributes FROM spans WHERE rowid IN ({rowid_list})");
        let mut spans = HashMap::new();
        self.scan_rows(&sql, &WhereClause::new(), |row| {
            let attributes: String = row.get(2)?;
            spans.insert(
                SpanRowid(row.get(0)?),
                SpanNameAndAttributes {
                    name: row.get(1)?,
                    attributes: serde_json::from_str(&attributes)?,
                },
            );
            Ok(ControlFlow::Continue(()))
        })?;
        Ok(spans)
    }
}

pub(super) fn group_spans(
    reader: &Reader,
    query: &Query,
    by: &[SpanGroupingField],
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<SpanGroups> {
    reader.check_step(step_ns)?;
    let (where_clause, unindexed) = compile_span_query(reader, query)?;
    let grouping_columns: String = by
        .iter()
        .flat_map(|field| [", ".to_owned(), grouping_column_as_json(field)])
        .collect();
    let sql = format!(
        "SELECT span.started_at, span.duration_ns, span.status_code, span.rowid{grouping_columns}
         FROM spans span
         JOIN resources resource ON resource.id = span.resource_id
         WHERE {}",
        where_clause.sql()
    );
    let mut all_spans = SpanTally::default();
    let mut steps: HashMap<i64, SpanTally> = HashMap::new();
    let mut groups: HashMap<GroupingValuesAsJson, GroupTally> = HashMap::new();
    reader.scan_rows(&sql, &where_clause, |row| {
        let started_at: i64 = row.get(0)?;
        let duration_ns: i64 = row.get(1)?;
        let failed = SpanStatus::from_number(row.get(2)?).is_error();
        let rowid = SpanRowid(row.get(3)?);
        let values = (0..by.len())
            .map(|index| row.get(4 + index))
            .collect::<rusqlite::Result<GroupingValuesAsJson>>()?;
        all_spans.add_span(duration_ns, failed);
        steps
            .entry(started_at.div_euclid(step_ns) * step_ns)
            .or_default()
            .add_span(duration_ns, failed);
        groups
            .entry(values)
            .or_default()
            .add_span(started_at, duration_ns, failed, rowid);
        Ok(ControlFlow::Continue(()))
    })?;

    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|(a_values, a), (b_values, b)| {
        b.spans
            .total_ns
            .cmp(&a.spans.total_ns)
            .then_with(|| a_values.cmp(b_values))
    });
    let truncated = truncate_to_limit(&mut groups, limit);
    let mut newest_spans = reader.read_span_names_and_attributes(
        groups
            .iter()
            .filter_map(|(_, group)| group.newest_span.map(|newest_span| newest_span.rowid)),
    )?;
    let groups = groups
        .into_iter()
        .map(|(values, group)| {
            let newest_span = group
                .newest_span
                .and_then(|newest_span| newest_spans.remove(&newest_span.rowid));
            let (name, attributes) = newest_span.map_or_else(
                || (String::new(), Attributes::new()),
                |span| (span.name, span.attributes),
            );
            let values = by
                .iter()
                .zip(values)
                .filter_map(|(field, value)| Some((field, value?)))
                .map(|(field, value)| {
                    let value: AttributeValue = serde_json::from_str(&value)?;
                    Ok((field.to_string(), value))
                })
                .collect::<anyhow::Result<Attributes>>()?;
            Ok(SpanGroup {
                values,
                name,
                attributes,
                spans: group.spans.to_span_stats(),
            })
        })
        .collect::<anyhow::Result<_>>()?;

    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    Ok(SpanGroups {
        start_at: timestamp_from_nanos(range.start_at()),
        end_at: timestamp_from_nanos(range.end_at()),
        step_ns,
        spans: all_spans.to_span_stats(),
        buckets: fill_span_buckets(&steps, first_step_at, range.end_at(), step_ns),
        groups,
        truncated,
        unindexed,
    })
}
