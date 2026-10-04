use std::collections::HashMap;
use std::fmt;
use std::ops::ControlFlow;

use otelo_indexed_storage::query::{
    GroupBuckets, SpanGroup, SpanGroupRanking, SpanGroupingField, SpanGroups,
};
use otelo_indexed_storage::{AttributeValue, Attributes, SpanStatus};
use otelo_query::Query;

use super::span_stats::{SpanTally, fill_span_buckets};
use super::traces::compile_span_query;
use super::{WhereClause, limit_groups_with_buckets, timestamp_from_nanos, truncate_to_limit};
use crate::Reader;
use crate::indexes::attribute_json_path;

pub(super) type GroupingValuesAsJson = Vec<Option<String>>;

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
        let rowids: Vec<i64> = rowids.map(|rowid| rowid.0).collect();
        if rowids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut where_clause = WhereClause::new();
        where_clause.push_param(":rowids", serde_json::to_string(&rowids)?);
        let mut spans = HashMap::new();
        let sql = "SELECT rowid, name, attributes
                   FROM spans
                   WHERE rowid IN (SELECT value FROM json_each(:rowids))";
        self.scan_rows(sql, &where_clause, |row| {
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
    ranking: SpanGroupRanking,
    step_ns: i64,
    group_buckets: GroupBuckets,
    limit: usize,
) -> anyhow::Result<SpanGroups> {
    reader.check_step(step_ns)?;
    let (where_clause, unindexed) = compile_span_query(reader, query)?;
    // The second read of the steps sees what the first saw, so the buckets of
    // a group add up to its count.
    let _snapshot = reader.connection().unchecked_transaction()?;
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
        let values = read_grouping_values(row, by.len())?;
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

    let mut groups = rank_groups(groups, ranking);
    let truncated = truncate_to_limit(
        &mut groups,
        count_kept_groups(reader, group_buckets, step_ns, limit),
    );
    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    let steps_of_kept_groups = match group_buckets {
        GroupBuckets::Omitted => None,
        GroupBuckets::Counted => Some(tally_steps_of_groups(
            reader,
            &sql,
            &where_clause,
            by.len(),
            step_ns,
            &groups.iter().map(|(values, _)| values).collect::<Vec<_>>(),
        )?),
    };
    let mut newest_spans = reader.read_span_names_and_attributes(
        groups
            .iter()
            .filter_map(|(_, group)| group.newest_span.map(|newest_span| newest_span.rowid)),
    )?;
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(index, (values, group))| {
            let newest_span = group
                .newest_span
                .and_then(|newest_span| newest_spans.remove(&newest_span.rowid));
            let (name, attributes) = newest_span.map_or_else(
                || (String::new(), Attributes::new()),
                |span| (span.name, span.attributes),
            );
            Ok(SpanGroup {
                values: parse_grouping_values(by, values)?,
                name,
                attributes,
                spans: group.spans.to_span_stats(),
                buckets: steps_of_kept_groups.as_ref().map(|steps_of_groups| {
                    fill_span_buckets(
                        &steps_of_groups[index],
                        first_step_at,
                        range.end_at(),
                        step_ns,
                    )
                }),
            })
        })
        .collect::<anyhow::Result<_>>()?;

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

fn rank_groups(
    groups: HashMap<GroupingValuesAsJson, GroupTally>,
    ranking: SpanGroupRanking,
) -> Vec<(GroupingValuesAsJson, GroupTally)> {
    let mut ranked_groups: Vec<_> = groups
        .into_iter()
        .map(|(values, group)| (group.spans.to_span_stats(), values, group))
        .collect();
    ranked_groups.sort_by(|(a_stats, a_values, _), (b_stats, b_values, _)| {
        ranking
            .compare(a_stats, b_stats)
            .then_with(|| a_values.cmp(b_values))
    });
    ranked_groups
        .into_iter()
        .map(|(_, values, group)| (values, group))
        .collect()
}

fn count_kept_groups(
    reader: &Reader,
    group_buckets: GroupBuckets,
    step_ns: i64,
    limit: usize,
) -> usize {
    match group_buckets {
        GroupBuckets::Omitted => limit,
        GroupBuckets::Counted => limit_groups_with_buckets(reader, step_ns, limit),
    }
}

pub(super) fn parse_grouping_values(
    by: &[impl fmt::Display],
    values: GroupingValuesAsJson,
) -> anyhow::Result<Attributes> {
    by.iter()
        .zip(values)
        .filter_map(|(field, value)| Some((field, value?)))
        .map(|(field, value)| {
            let value: AttributeValue = serde_json::from_str(&value)?;
            Ok((field.to_string(), value))
        })
        .collect()
}

fn read_grouping_values(
    row: &rusqlite::Row,
    field_count: usize,
) -> rusqlite::Result<GroupingValuesAsJson> {
    (0..field_count).map(|index| row.get(4 + index)).collect()
}

// A second read of the spans tallies the steps of the kept groups only, so the
// memory grows with the limit and not with the count of groups.
fn tally_steps_of_groups(
    reader: &Reader,
    sql: &str,
    where_clause: &WhereClause,
    field_count: usize,
    step_ns: i64,
    kept_groups: &[&GroupingValuesAsJson],
) -> anyhow::Result<Vec<HashMap<i64, SpanTally>>> {
    let group_indexes: HashMap<&GroupingValuesAsJson, usize> = kept_groups
        .iter()
        .enumerate()
        .map(|(index, values)| (*values, index))
        .collect();
    let mut steps_of_groups: Vec<HashMap<i64, SpanTally>> =
        kept_groups.iter().map(|_| HashMap::new()).collect();
    reader.scan_rows(sql, where_clause, |row| {
        let values = read_grouping_values(row, field_count)?;
        if let Some(&index) = group_indexes.get(&values) {
            let started_at: i64 = row.get(0)?;
            let failed = SpanStatus::from_number(row.get(2)?).is_error();
            steps_of_groups[index]
                .entry(started_at.div_euclid(step_ns) * step_ns)
                .or_default()
                .add_span(row.get(1)?, failed);
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(steps_of_groups)
}
