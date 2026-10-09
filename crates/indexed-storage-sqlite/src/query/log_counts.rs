use std::collections::HashMap;
use std::ops::ControlFlow;

use otelo_indexed_storage::Severity;
use otelo_indexed_storage::query::{
    LogCountBucket, LogCountGroup, LogCounts, LogGroupingField, RankOrder,
};
use otelo_query::Query;
use rusqlite::Row;

use super::logs::compile_log_query;
use super::span_groups::{GroupingValuesAsJson, parse_grouping_values};
use super::stored_attributes::StoredAttributes;
use super::{
    LOG_ALIASES, WhereClause, limit_groups_with_buckets, timestamp_from_nanos, truncate_to_limit,
};
use crate::Reader;
use crate::catalog::AttributeOwner;
use crate::indexes::attribute_json_path;

#[derive(Default)]
struct LogCountTally {
    count: u64,
    counts_by_step_start_at: HashMap<i64, u64>,
}

impl LogCountTally {
    fn add_lines(&mut self, step_start_at: i64, count: u64) {
        self.count += count;
        *self
            .counts_by_step_start_at
            .entry(step_start_at)
            .or_default() += count;
    }
}

// The JSON of a value keeps a bool apart from 1, which json_extract turns it into.
fn grouping_column(field: &LogGroupingField, stored_attributes: &StoredAttributes) -> String {
    match field {
        LogGroupingField::Service => "json_quote(resource.service)".to_owned(),
        LogGroupingField::Level => "log.severity_number".to_owned(),
        LogGroupingField::Attribute(key) => {
            LOG_ALIASES.attribute_json_sql(key, stored_attributes.stored_key(key))
        }
        LogGroupingField::Resource(key) => {
            format!("resource.attributes -> {}", attribute_json_path(key))
        }
    }
}

fn list_grouping_columns(reader: &Reader, by: &[LogGroupingField]) -> anyhow::Result<String> {
    let grouped_keys: Vec<&str> = by
        .iter()
        .filter_map(|field| match field {
            LogGroupingField::Attribute(key) => Some(key.as_str()),
            _ => None,
        })
        .collect();
    let stored_attributes =
        reader.read_stored_attributes(AttributeOwner::Log, &grouped_keys, &[])?;
    Ok(by
        .iter()
        .enumerate()
        .flat_map(|(index, field)| {
            [
                ", ".to_owned(),
                grouping_column(field, &stored_attributes),
                " AS ".to_owned(),
                grouping_alias(index),
            ]
        })
        .collect())
}

fn grouping_alias(index: usize) -> String {
    format!("grouping_value_{index}")
}

fn read_grouping_value_as_json(
    row: &Row,
    column_index: usize,
    field: &LogGroupingField,
) -> anyhow::Result<Option<String>> {
    if *field == LogGroupingField::Level {
        let severity = Severity::from_number(row.get(column_index)?);
        return Ok(Some(serde_json::to_string(
            &severity.level().to_ascii_lowercase(),
        )?));
    }
    Ok(row.get(column_index)?)
}

pub(super) fn count_logs(
    reader: &Reader,
    query: &Query,
    by: &[LogGroupingField],
    order: RankOrder,
    step_ns: i64,
    limit: usize,
) -> anyhow::Result<LogCounts> {
    reader.check_step(step_ns)?;
    let (mut where_clause, unindexed) = compile_log_query(reader, query)?;
    let grouping_columns = list_grouping_columns(reader, by)?;
    let grouping_aliases: String = (0..by.len())
        .flat_map(|index| [", ".to_owned(), grouping_alias(index)])
        .collect();
    let from_logs = format!(
        "FROM {}
         WHERE {}",
        LOG_ALIASES.encoded_records_sql("logs"),
        where_clause.sql()
    );
    // The second read of the steps sees what the first saw, so the buckets of
    // a group add up to its count.
    let _snapshot = reader.connection().unchecked_transaction()?;
    let group_by = if by.is_empty() {
        String::new()
    } else {
        format!("GROUP BY {}", &grouping_aliases[2..])
    };
    let mut kept_groups = count_lines_of_groups(
        reader,
        &format!(
            "SELECT count(*) AS line_count{grouping_columns}
             {from_logs}
             {group_by}"
        ),
        &where_clause,
        by,
    )?;
    kept_groups.sort_by(|(a_values, a_count), (b_values, b_count)| {
        order
            .orient_ordering(a_count.cmp(b_count))
            .then_with(|| a_values.cmp(b_values))
    });
    let truncated = truncate_to_limit(
        &mut kept_groups,
        limit_groups_with_buckets(reader, step_ns, limit),
    );

    where_clause.push_param(":step_ns", step_ns);
    let (all_lines, steps_of_groups) = tally_steps_of_logs(
        reader,
        &format!(
            "SELECT (log.logged_at / :step_ns) * :step_ns AS step_start_at{grouping_columns},
                count(*) AS line_count
             {from_logs}
             GROUP BY step_start_at{grouping_aliases}"
        ),
        &where_clause,
        by,
        &kept_groups,
    )?;

    let range = reader.range();
    let first_step_at = range.start_at().div_euclid(step_ns) * step_ns;
    let fill_buckets = |tally: &LogCountTally| {
        fill_log_count_buckets(
            &tally.counts_by_step_start_at,
            first_step_at,
            range.end_at(),
            step_ns,
        )
    };
    let groups = kept_groups
        .into_iter()
        .zip(&steps_of_groups)
        .map(|((values, count), steps)| {
            Ok(LogCountGroup {
                values: parse_grouping_values(by, values)?,
                count,
                buckets: fill_buckets(steps),
            })
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(LogCounts {
        start_at: timestamp_from_nanos(range.start_at()),
        end_at: timestamp_from_nanos(range.end_at()),
        step_ns,
        count: all_lines.count,
        buckets: fill_buckets(&all_lines),
        groups,
        truncated,
        unindexed,
    })
}

// A second read tallies the steps of the kept groups only, so the memory grows
// with the limit and not with the count of groups.
fn tally_steps_of_logs(
    reader: &Reader,
    sql: &str,
    where_clause: &WhereClause,
    by: &[LogGroupingField],
    kept_groups: &[(GroupingValuesAsJson, u64)],
) -> anyhow::Result<(LogCountTally, Vec<LogCountTally>)> {
    let group_indexes: HashMap<&GroupingValuesAsJson, usize> = kept_groups
        .iter()
        .enumerate()
        .map(|(index, (values, _))| (values, index))
        .collect();
    let mut all_lines = LogCountTally::default();
    let mut steps_of_groups: Vec<LogCountTally> = kept_groups
        .iter()
        .map(|_| LogCountTally::default())
        .collect();
    reader.scan_rows(sql, where_clause, |row| {
        let step_start_at: i64 = row.get(0)?;
        let count = u64::try_from(row.get::<_, i64>(1 + by.len())?)?;
        all_lines.add_lines(step_start_at, count);
        let values = read_grouping_values(row, 1, by)?;
        if let Some(&index) = group_indexes.get(&values) {
            steps_of_groups[index].add_lines(step_start_at, count);
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok((all_lines, steps_of_groups))
}

// A level is several severity numbers, so the rows of one group add up.
fn count_lines_of_groups(
    reader: &Reader,
    sql: &str,
    where_clause: &WhereClause,
    by: &[LogGroupingField],
) -> anyhow::Result<Vec<(GroupingValuesAsJson, u64)>> {
    let mut counts_of_groups: HashMap<GroupingValuesAsJson, u64> = HashMap::new();
    reader.scan_rows(sql, where_clause, |row| {
        let count = u64::try_from(row.get::<_, i64>(0)?)?;
        if count > 0 {
            *counts_of_groups
                .entry(read_grouping_values(row, 1, by)?)
                .or_default() += count;
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(counts_of_groups.into_iter().collect())
}

fn read_grouping_values(
    row: &Row,
    first_column_index: usize,
    by: &[LogGroupingField],
) -> anyhow::Result<GroupingValuesAsJson> {
    by.iter()
        .enumerate()
        .map(|(index, field)| read_grouping_value_as_json(row, first_column_index + index, field))
        .collect()
}

fn fill_log_count_buckets(
    counts_by_step_start_at: &HashMap<i64, u64>,
    first_step_at: i64,
    end_at: i64,
    step_ns: i64,
) -> Vec<LogCountBucket> {
    (first_step_at..end_at)
        .step_by(usize::try_from(step_ns).unwrap_or(usize::MAX))
        .map(|start_at| LogCountBucket {
            start_at: timestamp_from_nanos(start_at),
            count: counts_by_step_start_at
                .get(&start_at)
                .copied()
                .unwrap_or_default(),
        })
        .collect()
}
