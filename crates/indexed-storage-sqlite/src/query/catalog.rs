use otelo_indexed_storage::AttributeValue;
use otelo_indexed_storage::query::{Attribute, AttributeKeys};
use otelo_query::{
    BuiltinField, Catalog, Expression, Field, FieldValues, KeyInfo, Signal, Value, ValueInfo,
    ValueType,
};
use rusqlite::params;

use super::sample::{
    RecordSample, ValueColumn, ValuePrefixFilter, column_of_builtin_field, read_sampled_keys,
    read_sampled_values, read_whether_range_has_column, read_whether_sample_has_column,
    sample_records,
};
use crate::Reader;
use crate::catalog::{AttributeOwner, MAX_SPAN_NAMES_PER_DAY, json_type_from_stored_name};
use crate::completion_cache::ContextOfRange;
use crate::day::Day;

pub const MAX_CATALOG_ROWS: usize = 500;

#[derive(Clone, Copy)]
enum ListedColumn {
    ResourceService,
    SeriesName,
    SeriesUnit,
}

impl ListedColumn {
    const fn table_name(self) -> &'static str {
        match self {
            Self::ResourceService => "resources",
            Self::SeriesName | Self::SeriesUnit => "metric_series",
        }
    }

    const fn column_name(self) -> &'static str {
        match self {
            Self::ResourceService => "service",
            Self::SeriesName => "name",
            Self::SeriesUnit => "unit",
        }
    }
}

impl Reader {
    const fn days_of_range(&self) -> (Day, Day) {
        (
            Day::from_unix_nanos(self.range().start_at()),
            Day::from_unix_nanos(self.range().end_at() - 1),
        )
    }

    fn read_catalog_keys(&self, owner: AttributeOwner) -> anyhow::Result<Vec<KeyInfo>> {
        let (first_day, last_day) = self.days_of_range();
        let mut statement = self.connection().prepare(&format!(
            "SELECT key,
                    CASE WHEN count(DISTINCT json_type) > 1 THEN ?4 ELSE min(json_type) END,
                    sum(record_count) AS total_record_count
             FROM attribute_key_counts
             WHERE attribute_owner = ?1 AND day >= ?2 AND day <= ?3
             GROUP BY key
             ORDER BY total_record_count DESC, key
             LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = statement.query_map(
            params![owner.name(), first_day, last_day, ValueType::Mixed.name()],
            |row| {
                Ok(KeyInfo {
                    key: row.get(0)?,
                    value_type: json_type_from_stored_name(&row.get::<_, String>(1)?),
                    count: row.get::<_, i64>(2)?.cast_unsigned(),
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn read_catalog_values(&self, owner: AttributeOwner, key: &str) -> anyhow::Result<FieldValues> {
        let (first_day, last_day) = self.days_of_range();
        let mut statement = self.connection().prepare(&format!(
            "SELECT value, sum(record_count) AS total_record_count
             FROM attribute_value_counts
             WHERE attribute_owner = ?1 AND key = ?2 AND day >= ?3 AND day <= ?4
             GROUP BY value
             ORDER BY total_record_count DESC, value
             LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = statement.query_map(params![owner.name(), key, first_day, last_day], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut values = Vec::new();
        for row in rows {
            let (json, count) = row?;
            if let Some(value) = value_of_attribute_json(&json)? {
                values.push(ValueInfo {
                    value,
                    count: count.cast_unsigned(),
                });
            }
        }
        let has_more_values_than_listed = self.connection().query_row(
            "SELECT coalesce(max(has_more_values_than_listed), 0)
             FROM attribute_key_counts
             WHERE attribute_owner = ?1 AND key = ?2 AND day >= ?3 AND day <= ?4",
            params![owner.name(), key, first_day, last_day],
            |row| row.get(0),
        )?;
        Ok(FieldValues {
            listed: values,
            has_more_values_than_listed,
        })
    }

    fn read_span_names(&self) -> anyhow::Result<FieldValues> {
        let (first_day, last_day) = self.days_of_range();
        let mut statement = self.connection().prepare(&format!(
            "SELECT name, sum(record_count) AS total_record_count
             FROM span_name_counts
             WHERE day >= ?1 AND day <= ?2
             GROUP BY name
             ORDER BY total_record_count DESC, name
             LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = statement.query_map(params![first_day, last_day], |row| {
            Ok(ValueInfo {
                value: Value::String(row.get(0)?),
                count: row.get::<_, i64>(1)?.cast_unsigned(),
            })
        })?;
        let listed = rows.collect::<Result<_, _>>()?;
        // The indexer stops at this many names a day, so a day that reached it may have more.
        let has_more_values_than_listed = self.connection().query_row(
            &format!(
                "SELECT EXISTS (SELECT 1
                                FROM span_name_counts
                                WHERE day >= ?1 AND day <= ?2
                                GROUP BY day
                                HAVING count(*) >= {MAX_SPAN_NAMES_PER_DAY})"
            ),
            params![first_day, last_day],
            |row| row.get(0),
        )?;
        Ok(FieldValues {
            listed,
            has_more_values_than_listed,
        })
    }

    fn read_column_values(&self, listed_column: ListedColumn) -> anyhow::Result<FieldValues> {
        let (table, column) = (listed_column.table_name(), listed_column.column_name());
        let mut statement = self.connection().prepare(&format!(
            "SELECT {column}, count(*) AS row_count
             FROM {table}
             GROUP BY {column}
             ORDER BY row_count DESC, {column}
             LIMIT {}",
            MAX_CATALOG_ROWS + 1
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(ValueInfo {
                value: Value::String(row.get(0)?),
                count: row.get::<_, i64>(1)?.cast_unsigned(),
            })
        })?;
        let mut listed: Vec<ValueInfo> = rows.collect::<Result<_, _>>()?;
        let has_more_values_than_listed = listed.len() > MAX_CATALOG_ROWS;
        listed.truncate(MAX_CATALOG_ROWS);
        Ok(FieldValues {
            listed,
            has_more_values_than_listed,
        })
    }
}

impl Reader {
    fn read_catalog_field_values(
        &self,
        signal: Signal,
        field: &Field,
    ) -> anyhow::Result<FieldValues> {
        match field {
            Field::Attribute(key) => self.read_catalog_values(AttributeOwner::from(signal), key),
            Field::Resource(key) => self.read_catalog_values(AttributeOwner::Resource, key),
            Field::Builtin(BuiltinField::Service) => {
                self.read_column_values(ListedColumn::ResourceService)
            }
            Field::Builtin(BuiltinField::Name) if signal == Signal::Spans => self.read_span_names(),
            Field::Builtin(BuiltinField::Name) => self.read_column_values(ListedColumn::SeriesName),
            Field::Builtin(BuiltinField::Unit) => self.read_column_values(ListedColumn::SeriesUnit),
            Field::Builtin(_) => Ok(FieldValues::default()),
        }
    }

    fn context_of_range(&self, signal: Signal, context: &Expression) -> ContextOfRange {
        ContextOfRange {
            signal,
            context_text: context.to_string(),
            range: self.range(),
        }
    }

    fn sample_context(
        &self,
        context_of_range: &ContextOfRange,
        context: &Expression,
    ) -> anyhow::Result<RecordSample> {
        if let Some(sample) = self.completion_cache().find_sample(context_of_range) {
            return Ok(sample);
        }
        let sample = sample_records(self, context_of_range.signal, context, None)?;
        self.completion_cache()
            .keep_sample(context_of_range.clone(), sample.clone());
        Ok(sample)
    }

    fn read_keys_in_context(
        &self,
        signal: Signal,
        resource: bool,
        context: &Expression,
    ) -> anyhow::Result<Vec<KeyInfo>> {
        let owner = attribute_owner(signal, resource);
        let context_of_range = self.context_of_range(signal, context);
        if let Some(keys) = self.completion_cache().find_keys(&context_of_range, owner) {
            return Ok(keys);
        }
        let keys = match self.sample_context(&context_of_range, context)? {
            RecordSample::NothingFoundInTime => self.read_catalog_keys(owner)?,
            RecordSample::EveryMatch(rowids) | RecordSample::NewestMatches(rowids) => {
                read_sampled_keys(self, signal, resource, &rowids)?
            }
        };
        self.completion_cache()
            .keep_keys(&context_of_range, owner, keys.clone());
        Ok(keys)
    }

    fn read_values_of_context(
        &self,
        context_of_range: &ContextOfRange,
        sample: &RecordSample,
        field: &Field,
        value_column: &ValueColumn,
    ) -> anyhow::Result<FieldValues> {
        let cache = self.completion_cache();
        if let Some(values) = cache.find_values(context_of_range, field, "") {
            return Ok(values);
        }
        let signal = context_of_range.signal;
        let values = match sample {
            RecordSample::NothingFoundInTime => self.read_catalog_field_values(signal, field)?,
            sample => read_sampled_values(self, signal, value_column, sample)?,
        };
        cache.keep_values(context_of_range, field, "", values.clone());
        Ok(values)
    }

    fn read_whether_context_has_builtin_field(
        &self,
        signal: Signal,
        builtin_field: BuiltinField,
        context: Option<&Expression>,
    ) -> anyhow::Result<bool> {
        if builtin_field.is_on_every_record(signal) {
            return Ok(true);
        }
        let column = column_of_builtin_field(signal, builtin_field);
        let Some(context) = context else {
            return read_whether_range_has_column(self, signal, &column);
        };
        let context_of_range = self.context_of_range(signal, context);
        let cache = self.completion_cache();
        if let Some(has_field) =
            cache.find_whether_has_builtin_field(&context_of_range, builtin_field)
        {
            return Ok(has_field);
        }
        let has_field = match self.sample_context(&context_of_range, context)? {
            RecordSample::NothingFoundInTime => {
                read_whether_range_has_column(self, signal, &column)?
            }
            RecordSample::EveryMatch(rowids) | RecordSample::NewestMatches(rowids) => {
                read_whether_sample_has_column(self, signal, &column, &rowids)?
            }
        };
        cache.keep_whether_has_builtin_field(&context_of_range, builtin_field, has_field);
        Ok(has_field)
    }

    fn read_values_in_context(
        &self,
        signal: Signal,
        field: &Field,
        lowercase_value_prefix: &str,
        context: &Expression,
    ) -> anyhow::Result<FieldValues> {
        let Some(value_column) = ValueColumn::of_field(signal, field) else {
            return Ok(FieldValues::default());
        };
        let context_of_range = self.context_of_range(signal, context);
        let sample = self.sample_context(&context_of_range, context)?;
        let values_of_context =
            self.read_values_of_context(&context_of_range, &sample, field, &value_column)?;
        let needs_values_of_prefix = !lowercase_value_prefix.is_empty()
            && values_of_context.has_more_values_than_listed
            && !matches!(sample, RecordSample::NothingFoundInTime);
        if !needs_values_of_prefix {
            return Ok(values_of_context);
        }
        if let Some(values) =
            self.completion_cache()
                .find_values(&context_of_range, field, lowercase_value_prefix)
        {
            return Ok(values);
        }
        let sample_of_prefix = sample_records(
            self,
            signal,
            context,
            Some(&ValuePrefixFilter {
                value_column: &value_column,
                lowercase_value_prefix,
            }),
        )?;
        let values = match sample_of_prefix {
            RecordSample::NothingFoundInTime => values_of_context,
            sample_of_prefix => {
                read_sampled_values(self, signal, &value_column, &sample_of_prefix)?
            }
        };
        self.completion_cache().keep_values(
            &context_of_range,
            field,
            lowercase_value_prefix,
            values.clone(),
        );
        Ok(values)
    }
}

fn attribute_owner(signal: Signal, resource: bool) -> AttributeOwner {
    if resource {
        AttributeOwner::Resource
    } else {
        AttributeOwner::from(signal)
    }
}

pub fn value_of_attribute_json(json: &str) -> anyhow::Result<Option<Value>> {
    Ok(match serde_json::from_str(json)? {
        AttributeValue::String(text) => Some(Value::String(text)),
        AttributeValue::Bool(flag) => Some(Value::Bool(flag)),
        AttributeValue::Int(integer) => Some(Value::Int(integer)),
        AttributeValue::Double(real) => Some(Value::Float(real)),
        _ => None,
    })
}

fn read_or_warn<T: Default>(result: anyhow::Result<T>) -> T {
    result.unwrap_or_else(|error| {
        tracing::warn!("read the attribute catalog: {error:#}");
        T::default()
    })
}

impl Catalog for Reader {
    fn keys(&self, signal: Signal, resource: bool, context: Option<&Expression>) -> Vec<KeyInfo> {
        read_or_warn(context.map_or_else(
            || self.read_catalog_keys(attribute_owner(signal, resource)),
            |context| self.read_keys_in_context(signal, resource, context),
        ))
    }

    fn values(
        &self,
        signal: Signal,
        field: &Field,
        lowercase_value_prefix: &str,
        context: Option<&Expression>,
    ) -> FieldValues {
        read_or_warn(context.map_or_else(
            || self.read_catalog_field_values(signal, field),
            |context| self.read_values_in_context(signal, field, lowercase_value_prefix, context),
        ))
    }

    fn has_builtin_field(
        &self,
        signal: Signal,
        builtin_field: BuiltinField,
        context: Option<&Expression>,
    ) -> bool {
        self.read_whether_context_has_builtin_field(signal, builtin_field, context)
            .unwrap_or_else(|error| {
                tracing::warn!("read the attribute catalog: {error:#}");
                true
            })
    }
}

pub(super) fn list_attribute_keys(
    reader: &Reader,
    signal: Signal,
) -> anyhow::Result<AttributeKeys> {
    let is_indexed = |key: &str| {
        reader
            .indexed_attributes()
            .iter()
            .any(|attribute| Signal::from(attribute.signal()) == signal && attribute.key() == key)
    };
    let to_attributes = |keys: Vec<KeyInfo>, indexable: bool| {
        keys.into_iter()
            .map(|key_info| Attribute {
                indexed: indexable && is_indexed(&key_info.key),
                key: key_info.key,
                value_type: key_info.value_type,
                count: key_info.count,
            })
            .collect()
    };
    Ok(AttributeKeys {
        record: to_attributes(
            reader.read_catalog_keys(AttributeOwner::from(signal))?,
            true,
        ),
        resource: to_attributes(reader.read_catalog_keys(AttributeOwner::Resource)?, false),
    })
}
