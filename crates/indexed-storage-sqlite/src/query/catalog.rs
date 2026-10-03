use otelo_indexed_storage::AttributeValue;
use otelo_indexed_storage::query::{Attribute, AttributeKeys};
use otelo_query::{
    BuiltinField, Catalog, Field, FieldValues, KeyInfo, Signal, Value, ValueInfo, ValueType,
};
use rusqlite::params;

use crate::Reader;
use crate::catalog::{AttributeOwner, MAX_SPAN_NAMES_PER_DAY, json_type_from_stored_name};
use crate::day::Day;

const MAX_CATALOG_ROWS: usize = 500;

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
            let value = match serde_json::from_str(&json)? {
                AttributeValue::String(text) => Value::String(text),
                AttributeValue::Bool(flag) => Value::Bool(flag),
                AttributeValue::Int(integer) => Value::Int(integer),
                AttributeValue::Double(real) => Value::Float(real),
                _ => continue,
            };
            values.push(ValueInfo {
                value,
                count: count.cast_unsigned(),
            });
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
        // The writer stops at this many names a day, so a day that reached it may have more.
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

fn read_or_warn<T: Default>(result: anyhow::Result<T>) -> T {
    result.unwrap_or_else(|error| {
        tracing::warn!("read the attribute catalog: {error:#}");
        T::default()
    })
}

impl Catalog for Reader {
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo> {
        let owner = if resource {
            AttributeOwner::Resource
        } else {
            AttributeOwner::from(signal)
        };
        read_or_warn(self.read_catalog_keys(owner))
    }

    fn values(&self, signal: Signal, field: &Field) -> FieldValues {
        read_or_warn(match field {
            Field::Attribute(key) => self.read_catalog_values(AttributeOwner::from(signal), key),
            Field::Resource(key) => self.read_catalog_values(AttributeOwner::Resource, key),
            Field::Builtin(BuiltinField::Service) => {
                self.read_column_values(ListedColumn::ResourceService)
            }
            Field::Builtin(BuiltinField::Name) if signal == Signal::Spans => self.read_span_names(),
            Field::Builtin(BuiltinField::Name) => self.read_column_values(ListedColumn::SeriesName),
            Field::Builtin(BuiltinField::Unit) => self.read_column_values(ListedColumn::SeriesUnit),
            Field::Builtin(_) => Ok(FieldValues::default()),
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
