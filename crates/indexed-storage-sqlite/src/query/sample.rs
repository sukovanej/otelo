use std::collections::HashMap;
use std::ops::ControlFlow;

use otelo_indexed_storage::query::Resolution;
use otelo_query::{
    BuiltinField, Expression, Field, FieldValues, KeyInfo, Query, Signal, ValueInfo, ValueType,
};
use serde::Serialize;

use super::catalog::{MAX_CATALOG_ROWS, value_of_attribute_json};
use super::compile::{
    EncodedRecordAliases, QueryContext, TableAliases, builtin_column, compile_query,
};
use super::logs::compile_log_query;
use super::metrics::{SERIES_TABLE_ALIASES, where_series_have_points_in_range};
use super::stored_attributes::StoredAttributes;
use super::traces::compile_span_query;
use super::{LOG_ALIASES, SPAN_ALIASES, WhereClause, row_limit_with_one_more};
use crate::Reader;
use crate::catalog::AttributeOwner;
use crate::reader::timed_out;

// Every key and most values of mudro's spans show up in its newest 10,000, and reading their
// attributes takes about 50 ms.
pub const MAX_SAMPLED_RECORDS: usize = 10_000;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(transparent)]
pub struct SampledRowid(i64);

#[derive(Clone, Debug)]
pub enum RecordSample {
    EveryMatch(Vec<SampledRowid>),
    NewestMatches(Vec<SampledRowid>),
    NothingFoundInTime,
}

#[derive(Clone, Copy)]
enum SampledTable {
    EncodedRecords {
        table: &'static str,
        aliases: EncodedRecordAliases,
        order_newest_first: &'static str,
    },
    MetricSeries,
}

impl SampledTable {
    const fn of_signal(signal: Signal) -> Self {
        match signal {
            Signal::Logs => Self::EncodedRecords {
                table: "logs",
                aliases: LOG_ALIASES,
                order_newest_first: "log.logged_at DESC",
            },
            Signal::Spans => Self::EncodedRecords {
                table: "spans",
                aliases: SPAN_ALIASES,
                order_newest_first: "span.started_at DESC",
            },
            Signal::Metrics => Self::MetricSeries,
        }
    }

    const fn record(self) -> &'static str {
        match self {
            Self::EncodedRecords { aliases, .. } => aliases.record,
            Self::MetricSeries => "metric_series",
        }
    }

    const fn order_newest_first(self) -> &'static str {
        match self {
            Self::EncodedRecords {
                order_newest_first, ..
            } => order_newest_first,
            Self::MetricSeries => "metric_series.id DESC",
        }
    }

    fn records_sql(self) -> String {
        match self {
            Self::EncodedRecords { table, aliases, .. } => aliases.encoded_records_sql(table),
            Self::MetricSeries => "metric_series
                 JOIN resources resource ON resource.id = metric_series.resource_id"
                .to_owned(),
        }
    }
}

pub enum ValueColumn {
    RecordAttribute(String),
    ResourceAttribute(String),
    BuiltinColumn(String),
}

impl ValueColumn {
    pub fn of_field(signal: Signal, field: &Field) -> Option<Self> {
        Some(match (field, signal) {
            (Field::Attribute(key), _) if !key.contains('"') => Self::RecordAttribute(key.clone()),
            (Field::Resource(key), _) if !key.contains('"') => Self::ResourceAttribute(key.clone()),
            (
                Field::Builtin(
                    builtin_field @ (BuiltinField::Service
                    | BuiltinField::Name
                    | BuiltinField::Unit),
                ),
                _,
            ) => Self::BuiltinColumn(column_of_builtin_field(signal, *builtin_field)),
            _ => return None,
        })
    }

    fn read_stored_attributes(
        &self,
        reader: &Reader,
        signal: Signal,
    ) -> anyhow::Result<StoredAttributes> {
        match (self, signal) {
            (Self::RecordAttribute(key), Signal::Logs | Signal::Spans) => {
                reader.read_stored_attributes(AttributeOwner::from(signal), &[key], &[])
            }
            _ => Ok(StoredAttributes::default()),
        }
    }

    fn sql_reading_json(
        &self,
        table: SampledTable,
        stored_attributes: &StoredAttributes,
    ) -> String {
        match (self, table) {
            (Self::RecordAttribute(key), SampledTable::EncodedRecords { aliases, .. }) => {
                aliases.attribute_json_sql(key, stored_attributes.stored_key(key))
            }
            (Self::RecordAttribute(_), SampledTable::MetricSeries) => {
                "metric_series.attributes -> :value_path".into()
            }
            (Self::ResourceAttribute(_), _) => "resource.attributes -> :value_path".into(),
            (Self::BuiltinColumn(column), _) => column.clone(),
        }
    }

    fn sql_reading_text(
        &self,
        table: SampledTable,
        stored_attributes: &StoredAttributes,
    ) -> String {
        match (self, table) {
            (Self::RecordAttribute(key), SampledTable::EncodedRecords { aliases, .. }) => {
                aliases.attribute_text_sql(key, stored_attributes.stored_key(key))
            }
            (Self::RecordAttribute(_), SampledTable::MetricSeries) => {
                "metric_series.attributes ->> :value_path".into()
            }
            (Self::ResourceAttribute(_), _) => "resource.attributes ->> :value_path".into(),
            (Self::BuiltinColumn(column), _) => column.clone(),
        }
    }

    const fn holds_json(&self) -> bool {
        !matches!(self, Self::BuiltinColumn(_))
    }

    fn push_path_param(&self, where_clause: &mut WhereClause, table: SampledTable) {
        match (self, table) {
            (Self::RecordAttribute(_), SampledTable::EncodedRecords { .. })
            | (Self::BuiltinColumn(_), _) => {}
            (Self::RecordAttribute(key) | Self::ResourceAttribute(key), _) => {
                where_clause.push_param(":value_path", format!("$.\"{key}\""));
            }
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
    let table = SampledTable::of_signal(signal);
    let mut where_clause = compile_context(reader, signal, Some(context))?;
    if let Some(ValuePrefixFilter {
        value_column,
        lowercase_value_prefix,
    }) = value_prefix_filter
    {
        let stored_attributes = value_column.read_stored_attributes(reader, signal)?;
        where_clause.push_condition_with_param(
            &format!(
                "{} LIKE :value_prefix ESCAPE '\\'",
                value_column.sql_reading_text(table, &stored_attributes)
            ),
            ":value_prefix",
            format!("{}%", escape_like_pattern(lowercase_value_prefix)),
        );
        value_column.push_path_param(&mut where_clause, table);
    }
    let sql = format!(
        "SELECT {}.rowid
         FROM {}
         WHERE {}
         ORDER BY {}
         LIMIT :limit",
        table.record(),
        table.records_sql(),
        where_clause.sql(),
        table.order_newest_first()
    );
    where_clause.push_param(":limit", i64::try_from(MAX_SAMPLED_RECORDS)?);
    let mut rowids = Vec::new();
    let scanned = reader.run_within_completion_time_budget(|| {
        reader.scan_rows(&sql, &where_clause, |row| {
            rowids.push(SampledRowid(row.get(0)?));
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
            let mut where_clause = where_series_have_points_in_range(reader, Resolution::Raw);
            compile_query(
                &query,
                &QueryContext {
                    aliases: SERIES_TABLE_ALIASES,
                    indexed_attributes: reader.indexed_attributes(),
                    stored_attributes: &StoredAttributes::default(),
                },
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

fn where_rowids_are(rowids: &[SampledRowid]) -> anyhow::Result<WhereClause> {
    let mut where_clause = WhereClause::new();
    where_clause.push_param(":rowids", serde_json::to_string(rowids)?);
    Ok(where_clause)
}

pub fn read_sampled_keys(
    reader: &Reader,
    signal: Signal,
    resource: bool,
    rowids: &[SampledRowid],
) -> anyhow::Result<Vec<KeyInfo>> {
    let table = SampledTable::of_signal(signal);
    let (record, records_sql) = (table.record(), table.records_sql());
    let sql = match (resource, table) {
        (true, _) => format!(
            "SELECT attribute.key, attribute.type, count(*) AS owner_count
             FROM resources owner_resource, json_each(owner_resource.attributes) attribute
             WHERE owner_resource.id IN (
               SELECT resource.id
               FROM {records_sql}
               WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids)))
             GROUP BY attribute.key, attribute.type"
        ),
        (false, SampledTable::EncodedRecords { aliases, .. }) => {
            let stable_attribute_set = aliases.stable_attribute_set;
            format!(
                "SELECT attribute.key, attribute.type, count(*) AS owner_count
                 FROM (SELECT stable_entry.key AS key, stable_entry.type AS type
                       FROM {records_sql}, json_each({stable_attribute_set}.attributes) stable_entry
                       WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))
                       UNION ALL
                       SELECT attribute_key.key, json_type(interned_attribute_value.value)
                       FROM {records_sql}, json_each({record}.interned_attributes) interned_entry
                       JOIN attribute_keys attribute_key
                         ON attribute_key.id = CAST(interned_entry.key AS INTEGER)
                       JOIN interned_attribute_values interned_attribute_value
                         ON interned_attribute_value.id = interned_entry.value
                       WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))
                       UNION ALL
                       SELECT attribute_key.key, literal_entry.type
                       FROM {records_sql}, json_each({record}.literal_attributes) literal_entry
                       JOIN attribute_keys attribute_key
                         ON attribute_key.id = CAST(literal_entry.key AS INTEGER)
                       WHERE {record}.rowid IN (SELECT value FROM json_each(:rowids))) attribute
                 GROUP BY attribute.key, attribute.type"
            )
        }
        (false, SampledTable::MetricSeries) => {
            "SELECT attribute.key, attribute.type, count(*) AS owner_count
             FROM metric_series, json_each(metric_series.attributes) attribute
             WHERE metric_series.rowid IN (SELECT value FROM json_each(:rowids))
             GROUP BY attribute.key, attribute.type"
                .to_owned()
        }
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
    let table = SampledTable::of_signal(signal);
    let stored_attributes = value_column.read_stored_attributes(reader, signal)?;
    let sql = format!(
        "SELECT {} AS field_value, count(*) AS record_count
         FROM {}
         WHERE {}.rowid IN (SELECT value FROM json_each(:rowids))
         GROUP BY field_value
         HAVING field_value IS NOT NULL
         ORDER BY record_count DESC, field_value
         LIMIT :limit",
        value_column.sql_reading_json(table, &stored_attributes),
        table.records_sql(),
        table.record()
    );
    let mut where_clause = where_rowids_are(rowids)?;
    value_column.push_path_param(&mut where_clause, table);
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

pub fn column_of_builtin_field(signal: Signal, builtin_field: BuiltinField) -> String {
    let aliases = match SampledTable::of_signal(signal) {
        SampledTable::EncodedRecords { aliases, .. } => TableAliases::EncodedRecords(aliases),
        SampledTable::MetricSeries => SERIES_TABLE_ALIASES,
    };
    builtin_column(aliases, builtin_field)
}

pub fn read_whether_sample_has_column(
    reader: &Reader,
    signal: Signal,
    column: &str,
    rowids: &[SampledRowid],
) -> anyhow::Result<bool> {
    let table = SampledTable::of_signal(signal);
    let sql = format!(
        "SELECT EXISTS (SELECT 1
                        FROM {}
                        WHERE {}.rowid IN (SELECT value FROM json_each(:rowids))
                          AND {column} IS NOT NULL)",
        table.records_sql(),
        table.record()
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
    let table = SampledTable::of_signal(signal);
    let mut where_clause = compile_context(reader, signal, None)?;
    where_clause.push_condition(format!("{column} IS NOT NULL"));
    let sql = format!(
        "SELECT EXISTS (SELECT 1
                        FROM {}
                        WHERE {})",
        table.records_sql(),
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
