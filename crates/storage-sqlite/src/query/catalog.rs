use otelo_query::{Builtin, Catalog, Field, KeyInfo, Signal, Value, ValueInfo};
use otelo_storage::AttributeValue;
use otelo_storage::query::{Attribute, AttributeKeys};

use crate::Reader;

const MAX_CATALOG_ROWS: usize = 500;

impl Reader {
    fn read_catalog_keys(&self, group: &str) -> anyhow::Result<Vec<KeyInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT key,
                    CASE WHEN count(DISTINCT type) > 1 THEN 'mixed' ELSE min(type) END,
                    sum(count)
             FROM attribute_keys WHERE signal = ?1
             GROUP BY key ORDER BY 3 DESC, key LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = stmt.query_map([group], |row| {
            Ok(KeyInfo {
                key: row.get(0)?,
                kind: row.get(1)?,
                count: row.get::<_, i64>(2)?.cast_unsigned(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn read_catalog_values(&self, group: &str, key: &str) -> anyhow::Result<Vec<ValueInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT value, sum(count) FROM attribute_values WHERE signal = ?1 AND key = ?2
             GROUP BY value ORDER BY 2 DESC, value LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = stmt.query_map([group, key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut values = Vec::new();
        for row in rows {
            let (json, count) = row?;
            let value = match serde_json::from_str(&json)? {
                AttributeValue::String(text) => Value::String(text),
                AttributeValue::Bool(b) => Value::Bool(b),
                AttributeValue::Int(n) => Value::Int(n),
                AttributeValue::Double(x) => Value::Float(x),
                _ => continue,
            };
            values.push(ValueInfo {
                value,
                count: count.cast_unsigned(),
            });
        }
        Ok(values)
    }

    fn read_column_values(&self, table: &str, column: &str) -> anyhow::Result<Vec<ValueInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {column}, count(*) FROM {table}
             GROUP BY {column} ORDER BY 2 DESC, 1 LIMIT {MAX_CATALOG_ROWS}"
        ))?;
        let rows = stmt.query_map([], |row| {
            Ok(ValueInfo {
                value: Value::String(row.get(0)?),
                count: row.get::<_, i64>(1)?.cast_unsigned(),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn values_or_warn<T>(result: anyhow::Result<Vec<T>>) -> Vec<T> {
    result.unwrap_or_else(|error| {
        tracing::warn!("read the attribute catalog: {error:#}");
        Vec::new()
    })
}

impl Catalog for Reader {
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo> {
        let group = if resource {
            "resource"
        } else {
            signal.as_str()
        };
        values_or_warn(self.read_catalog_keys(group))
    }

    fn values(&self, signal: Signal, field: &Field) -> Vec<ValueInfo> {
        values_or_warn(match field {
            Field::Attribute(key) => self.read_catalog_values(signal.as_str(), key),
            Field::Resource(key) => self.read_catalog_values("resource", key),
            Field::Builtin(Builtin::Service) => self.read_column_values("resources", "service"),
            Field::Builtin(Builtin::Name) if signal == Signal::Spans => {
                self.read_catalog_values("span_names", "name")
            }
            Field::Builtin(Builtin::Name) => self.read_column_values("series", "name"),
            Field::Builtin(Builtin::Unit) => self.read_column_values("series", "unit"),
            Field::Builtin(_) => Ok(Vec::new()),
        })
    }
}

pub(super) fn list_attribute_keys(
    reader: &Reader,
    signal: Signal,
) -> anyhow::Result<AttributeKeys> {
    let indexed = |key: &str| {
        reader
            .indexed_attributes()
            .iter()
            .any(|attribute| attribute.signal().signal() == signal && attribute.key() == key)
    };
    let to_attributes = |keys: Vec<KeyInfo>, record: bool| {
        keys.into_iter()
            .map(|key| Attribute {
                indexed: record && indexed(&key.key),
                key: key.key,
                kind: key.kind,
                count: key.count,
            })
            .collect()
    };
    Ok(AttributeKeys {
        record: to_attributes(reader.read_catalog_keys(signal.as_str())?, true),
        resource: to_attributes(reader.read_catalog_keys("resource")?, false),
    })
}
