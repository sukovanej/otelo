use std::collections::HashMap;
use std::ops::ControlFlow;

use otelo_query::{
    BuiltinField, Expression, Field, FieldValues, KeyInfo, Query, Signal, ValueInfo, ValueType,
};

use super::catalog::{MAX_CATALOG_ROWS, value_of_attribute_json};
use super::compile::compile_query;
use super::logs::compile_log_query;
use super::metrics::SERIES_TABLE_ALIASES;
use super::traces::compile_span_query;
use super::{WhereClause, row_limit_with_one_more};
use crate::Reader;
use crate::reader::timed_out;

// Every key and most values of mudro's spans show up in its newest 10,000, and reading their
// attributes takes about 50 ms.
pub const MAX_SAMPLED_RECORDS: usize = 10_000;

#[derive(Clone, Debug)]
pub enum RecordSample {
    EveryMatch(Vec<i64>),
    NewestMatches(Vec<i64>),
    NothingFoundInTime,
}

#[derive(Clone, Copy)]
struct SampledTable {
    table_with_alias: &'static str,
    record: &'static str,
    order_newest_first: &'static str,
}

const fn sampled_table_of_signal(signal: Signal) -> SampledTable {
    match signal {
        Signal::Logs => SampledTable {
            table_with_alias: "logs log",
            record: "log",
            order_newest_first: "log.logged_at DESC",
        },
        Signal::Spans => SampledTable {
            table_with_alias: "spans span",
            record: "span",
            order_newest_first: "span.started_at DESC",
        },
        Signal::Metrics => SampledTable {
            table_with_alias: "metric_series",
            record: "metric_series",
            order_newest_first: "metric_series.id DESC",
        },
    }
}

pub enum ValueColumn {
    RecordAttribute(String),
    ResourceAttribute(String),
    Column(&'static str),
}

impl ValueColumn {
    pub fn of_field(signal: Signal, field: &Field) -> Option<Self> {
        Some(match (field, signal) {
            (Field::Attribute(key), _) if !key.contains('"') => Self::RecordAttribute(key.clone()),
            (Field::Resource(key), _) if !key.contains('"') => Self::ResourceAttribute(key.clone()),
            (Field::Builtin(BuiltinField::Service), _) => Self::Column("resource.service"),
            (Field::Builtin(BuiltinField::Name), Signal::Spans) => Self::Column("span.name"),
            (Field::Builtin(BuiltinField::Name), Signal::Metrics) => {
                Self::Column("metric_series.name")
            }
            (Field::Builtin(BuiltinField::Unit), Signal::Metrics) => {
                Self::Column("metric_series.unit")
            }
            _ => return None,
        })
    }

    fn json_sql(&self, record: &str) -> String {
        match self {
            Self::RecordAttribute(_) => format!("{record}.attributes -> :value_path"),
            Self::ResourceAttribute(_) => "resource.attributes -> :value_path".into(),
            Self::Column(column) => (*column).into(),
        }
    }

    fn text_sql(&self, record: &str) -> String {
        match self {
            Self::RecordAttribute(_) => format!("{record}.attributes ->> :value_path"),
            Self::ResourceAttribute(_) => "resource.attributes ->> :value_path".into(),
            Self::Column(column) => (*column).into(),
        }
    }

    const fn holds_json(&self) -> bool {
        !matches!(self, Self::Column(_))
    }

    fn push_path_param(&self, where_clause: &mut WhereClause) {
        if let Self::RecordAttribute(key) | Self::ResourceAttribute(key) = self {
            where_clause.push_param(":value_path", format!("$.\"{key}\""));
        }
    }
}

pub struct ValuePrefixFilter<'a> {
    pub value_column: &'a ValueColumn,
    pub lowercase_value_prefix: &'a str,
}

pub fn sample_records(
    reader: &Reader,
    signal: Signal,
    context: &Expression,
    value_prefix_filter: Option<&ValuePrefixFilter>,
) -> anyhow::Result<RecordSample> {
    let SampledTable {
        table_with_alias,
        record,
        order_newest_first,
    } = sampled_table_of_signal(signal);
    let mut where_clause = compile_context(reader, signal, Some(context))?;
    if let Some(ValuePrefixFilter {
        value_column,
        lowercase_value_prefix,
    }) = value_prefix_filter
    {
        where_clause.push_condition_with_param(
            &format!(
                "{} LIKE :value_prefix ESCAPE '\\'",
                value_column.text_sql(record)
            ),
            ":value_prefix",
            format!("{}%", escape_like_pattern(lowercase_value_prefix)),
        );
        value_column.push_path_param(&mut where_clause);
    }
    let sql = format!(
        "SELECT {record}.rowid
         FROM {table_with_alias}
         JOIN resources resource ON resource.id = {record}.resource_id
         WHERE {}
         ORDER BY {order_newest_first}
         LIMIT :limit",
        where_clause.sql()
    );
    where_clause.push_param(":limit", i64::try_from(MAX_SAMPLED_RECORDS)?);
    let mut rowids = Vec::new();
    let scanned = reader.run_within_completion_time_budget(|| {
        reader.scan_rows(&sql, &where_clause, |row| {
            rowids.push(row.get(0)?);
            Ok(ControlFlow::Continue(()))
        })
    })?;
    Ok(match scanned {
        Ok(()) if rowids.len() < MAX_SAMPLED_RECORDS => RecordSample::EveryMatch(rowids),
        Ok(()) => RecordSample::NewestMatches(rowids),
        Err(error) if timed_out(&error) && rowids.is_empty() => RecordSample::NothingFoundInTime,
        Err(error) if timed_out(&error) => RecordSample::NewestMatches(rowids),
        Err(error) => return Err(error),
    })
}

fn compile_context(
    reader: &Reader,
    signal: Signal,
    context: Option<&Expression>,
) -> anyhow::Result<WhereClause> {
    let query = Query {
        signal,
        expression: context.cloned(),
    };
    Ok(match signal {
        Signal::Logs => compile_log_query(reader, &query)?.0,
        Signal::Spans => compile_span_query(reader, &query)?.0,
        Signal::Metrics => {
            let mut where_clause = WhereClause::new();
            where_clause.push_condition(
                "EXISTS (SELECT 1
                         FROM metric_points metric_point
                         WHERE metric_point.metric_series_id = metric_series.id
                           AND metric_point.recorded_at >= :since
                           AND metric_point.recorded_at < :until)"
                    .into(),
            );
            where_clause.push_param(":since", reader.range().start_at());
            where_clause.push_param(":until", reader.range().end_at());
            compile_query(
                &query,
                SERIES_TABLE_ALIASES,
                reader.indexed_attributes(),
                "q",
                &mut where_clause,
            )?;
            where_clause
        }
    })
}

fn escape_like_pattern(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn where_rowids_are(rowids: &[i64]) -> anyhow::Result<WhereClause> {
    let mut where_clause = WhereClause::new();
    where_clause.push_param(":rowids", serde_json::to_string(rowids)?);
    Ok(where_clause)
}

pub fn read_sampled_keys(
    reader: &Reader,
    signal: Signal,
    resource: bool,
    rowids: &[i64],
) -> anyhow::Result<Vec<KeyInfo>> {
    let SampledTable {
        table_with_alias,
        record,
        ..
    } = sampled_table_of_signal(signal);
    let sql = if resource {
        format!(
            "SELECT attribute.key, attribute.type, count(*) AS owner_count
             FROM resources resource, json_each(resource.attributes) attribute
             WHERE resource.id IN (SELECT {record}.resource_id
                                   FROM {table_with_alias}
                                   WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids)))
             GROUP BY attribute.key, attribute.type"
        )
    } else {
        format!(
            "SELECT attribute.key, attribute.type, count(*) AS owner_count
             FROM {table_with_alias}, json_each({record}.attributes) attribute
             WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))
             GROUP BY attribute.key, attribute.type"
        )
    };
    let rows = reader.collect_rows(&sql, &where_rowids_are(rowids)?, |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut keys_by_name: HashMap<String, KeyInfo> = HashMap::new();
    for (key, json_each_type, owner_count) in rows {
        let value_type = value_type_of_json_each_type(&json_each_type);
        let owner_count = owner_count.cast_unsigned();
        keys_by_name
            .entry(key.clone())
            .and_modify(|info| {
                if info.value_type != value_type {
                    info.value_type = ValueType::Mixed;
                }
                info.count += owner_count;
            })
            .or_insert(KeyInfo {
                key,
                value_type,
                count: owner_count,
            });
    }
    let mut keys: Vec<KeyInfo> = keys_by_name.into_values().collect();
    keys.sort_by(|first, second| {
        second
            .count
            .cmp(&first.count)
            .then_with(|| first.key.cmp(&second.key))
    });
    keys.truncate(MAX_CATALOG_ROWS);
    Ok(keys)
}

fn value_type_of_json_each_type(json_each_type: &str) -> ValueType {
    match json_each_type {
        "null" => ValueType::Null,
        "true" | "false" => ValueType::Bool,
        "integer" => ValueType::Int,
        "real" => ValueType::Float,
        "text" => ValueType::String,
        "array" => ValueType::Array,
        "object" => ValueType::Object,
        _ => ValueType::Mixed,
    }
}

pub fn read_sampled_values(
    reader: &Reader,
    signal: Signal,
    value_column: &ValueColumn,
    sample: &RecordSample,
) -> anyhow::Result<FieldValues> {
    let (rowids, sample_has_every_match) = match sample {
        RecordSample::EveryMatch(rowids) => (rowids.as_slice(), true),
        RecordSample::NewestMatches(rowids) => (rowids.as_slice(), false),
        RecordSample::NothingFoundInTime => return Ok(FieldValues::default()),
    };
    let SampledTable {
        table_with_alias,
        record,
        ..
    } = sampled_table_of_signal(signal);
    let sql = format!(
        "SELECT {} AS field_value, count(*) AS record_count
         FROM {table_with_alias}
         JOIN resources resource ON resource.id = {record}.resource_id
         WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))
         GROUP BY field_value
         HAVING field_value IS NOT NULL
         ORDER BY record_count DESC, field_value
         LIMIT :limit",
        value_column.json_sql(record)
    );
    let mut where_clause = where_rowids_are(rowids)?;
    value_column.push_path_param(&mut where_clause);
    where_clause.push_param(":limit", row_limit_with_one_more(MAX_CATALOG_ROWS)?);
    let rows = reader.collect_rows(&sql, &where_clause, |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let has_more_rows_than_listed = rows.len() > MAX_CATALOG_ROWS;
    let mut listed = Vec::new();
    for (text, record_count) in rows.into_iter().take(MAX_CATALOG_ROWS) {
        let value = if value_column.holds_json() {
            match value_of_attribute_json(&text)? {
                Some(value) => value,
                None => continue,
            }
        } else {
            otelo_query::Value::String(text)
        };
        listed.push(ValueInfo {
            value,
            count: record_count.cast_unsigned(),
        });
    }
    Ok(FieldValues {
        listed,
        has_more_values_than_listed: has_more_rows_than_listed || !sample_has_every_match,
    })
}

pub const fn find_column_of_builtin_field_some_records_lack(
    signal: Signal,
    builtin_field: BuiltinField,
) -> Option<&'static str> {
    match (builtin_field, signal) {
        (BuiltinField::TraceId, Signal::Logs) => Some("log.trace_id"),
        (BuiltinField::SpanId, Signal::Logs) => Some("log.span_id"),
        _ => None,
    }
}

pub fn read_whether_sample_has_column(
    reader: &Reader,
    signal: Signal,
    column: &str,
    rowids: &[i64],
) -> anyhow::Result<bool> {
    let SampledTable {
        table_with_alias,
        record,
        ..
    } = sampled_table_of_signal(signal);
    let sql = format!(
        "SELECT EXISTS (SELECT 1
                        FROM {table_with_alias}
                        WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))
                          AND {column} IS NOT NULL)"
    );
    let has_column = reader.collect_rows(&sql, &where_rowids_are(rowids)?, |row| {
        Ok(row.get::<_, bool>(0)?)
    })?;
    Ok(has_column.first().copied().unwrap_or(false))
}

// A range that shows no record with the column before the budget ends counts as having one.
pub fn read_whether_range_has_column(
    reader: &Reader,
    signal: Signal,
    column: &str,
) -> anyhow::Result<bool> {
    let SampledTable {
        table_with_alias,
        record,
        ..
    } = sampled_table_of_signal(signal);
    let mut where_clause = compile_context(reader, signal, None)?;
    where_clause.push_condition(format!("{column} IS NOT NULL"));
    let sql = format!(
        "SELECT EXISTS (SELECT 1
                        FROM {table_with_alias}
                        JOIN resources resource ON resource.id = {record}.resource_id
                        WHERE {})",
        where_clause.sql()
    );
    let found = reader.run_within_completion_time_budget(|| {
        reader.collect_rows(&sql, &where_clause, |row| Ok(row.get::<_, bool>(0)?))
    })?;
    match found {
        Ok(found) => Ok(found.first().copied().unwrap_or(false)),
        Err(error) if timed_out(&error) => Ok(true),
        Err(error) => Err(error),
    }
}
