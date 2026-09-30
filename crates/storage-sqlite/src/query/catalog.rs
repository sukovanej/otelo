use otelo_query::{BuiltinField, Catalog, Field, FieldValues, KeyInfo, Signal, Value, ValueInfo};
use otelo_storage::AttributeValue;
use otelo_storage::query::{Attribute, AttributeKeys};

use crate::Reader;
use crate::catalog::{KeyGroup, value_type_from_stored_name};

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
            Self::SeriesName | Self::SeriesUnit => "series",
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
    fn read_catalog_keys(&self, group: KeyGroup) -> anyhow::Result<Vec<KeyInfo>> {
        let mut statement = self.connection().prepare(&format!(
            "SELECT key,
                    CASE WHEN count(DISTINCT value_type) > 1 THEN 'mixed' ELSE min(value_type) END,
                    sum(count) AS record_count
             FROM attribute_keys
             WHERE key_group = ?1
             GROUP BY key
             ORDER BY record_count DESC, key
             LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = statement.query_map([group.name()], |row| {
            Ok(KeyInfo {
                key: row.get(0)?,
                value_type: value_type_from_stored_name(&row.get::<_, String>(1)?),
                count: row.get::<_, i64>(2)?.cast_unsigned(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn read_catalog_values(&self, group: KeyGroup, key: &str) -> anyhow::Result<FieldValues> {
        let mut statement = self.connection().prepare(&format!(
            "SELECT value, sum(count) AS record_count
             FROM attribute_values
             WHERE key_group = ?1 AND key = ?2
             GROUP BY value
             ORDER BY record_count DESC, value
             LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = statement.query_map([group.name(), key], |row| {
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
            "SELECT coalesce(max(has_more_values), 0)
             FROM attribute_keys
             WHERE key_group = ?1 AND key = ?2",
            [group.name(), key],
            |row| row.get(0),
        )?;
        Ok(FieldValues {
            listed: values,
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
        let group = if resource {
            KeyGroup::Resource
        } else {
            KeyGroup::from(signal)
        };
        read_or_warn(self.read_catalog_keys(group))
    }

    fn values(&self, signal: Signal, field: &Field) -> FieldValues {
        read_or_warn(match field {
            Field::Attribute(key) => self.read_catalog_values(KeyGroup::from(signal), key),
            Field::Resource(key) => self.read_catalog_values(KeyGroup::Resource, key),
            Field::Builtin(BuiltinField::Service) => {
                self.read_column_values(ListedColumn::ResourceService)
            }
            Field::Builtin(BuiltinField::Name) if signal == Signal::Spans => {
                self.read_catalog_values(KeyGroup::SpanNames, "name")
            }
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
        record: to_attributes(reader.read_catalog_keys(KeyGroup::from(signal))?, true),
        resource: to_attributes(reader.read_catalog_keys(KeyGroup::Resource)?, false),
    })
}
