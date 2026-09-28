use serde::{Deserialize, Serialize};
use siner_query::{Builtin, Catalog, Field, KeyInfo, Signal, Value, ValueInfo};
use utoipa::ToSchema;

use crate::{AttributeValue, Reader};

/// The most keys or values one catalog read returns.
const MAX_ROWS: usize = 500;

/// The attribute keys of a signal, over the attached days.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct AttributeKeys {
    /// The attributes of the records: of the logs, the spans, or the labels
    /// of the series.
    pub record: Vec<Attribute>,
    /// The attributes of the resources that sent them.
    pub resource: Vec<Attribute>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Attribute {
    pub key: String,
    /// The JSON type of the values: `string`, `int`, `float`, `bool`,
    /// `array`, `object`, or `mixed`.
    #[serde(rename = "type")]
    pub kind: String,
    /// How many records have the key. For resources and series, how many
    /// resources and series.
    pub count: u64,
    /// Whether the key has an index.
    pub indexed: bool,
}

impl Reader {
    /// The attribute keys of the records of `signal` and of their resources,
    /// the most common first.
    ///
    /// # Errors
    ///
    /// When the catalog cannot be read.
    pub fn attributes(&self, signal: Signal) -> anyhow::Result<AttributeKeys> {
        let indexed = |key: &str| {
            self.indexes()
                .iter()
                .any(|k| k.signal == signal && k.key == key)
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
            record: to_attributes(self.catalog_keys(signal.as_str())?, true),
            resource: to_attributes(self.catalog_keys("resource")?, false),
        })
    }

    /// The keys of a catalog group, the most common first.
    fn catalog_keys(&self, group: &str) -> anyhow::Result<Vec<KeyInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT key,
                    CASE WHEN count(DISTINCT type) > 1 THEN 'mixed' ELSE min(type) END,
                    sum(count)
             FROM attribute_keys WHERE signal = ?1
             GROUP BY key ORDER BY 3 DESC, key LIMIT {MAX_ROWS}"
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

    /// The values the catalog keeps of `key` in a group, the most common
    /// first.
    fn catalog_values(&self, group: &str, key: &str) -> anyhow::Result<Vec<ValueInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT value, sum(count) FROM attribute_values WHERE signal = ?1 AND key = ?2
             GROUP BY value ORDER BY 2 DESC, value LIMIT {MAX_ROWS}"
        ))?;
        let rows = stmt.query_map([group, key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        let mut values = Vec::new();
        for row in rows {
            let (json, count) = row?;
            // The catalog keeps each value as the JSON of the attribute.
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

    /// The values of a text column over the attached days, the most common
    /// first.
    fn column_values(&self, table: &str, column: &str) -> anyhow::Result<Vec<ValueInfo>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {column}, count(*) FROM {table}
             GROUP BY {column} ORDER BY 2 DESC, 1 LIMIT {MAX_ROWS}"
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

/// The catalog of the day files a reader attached, for
/// [`siner_query::complete`].
pub struct ReaderCatalog<'a>(pub &'a Reader);

impl ReaderCatalog<'_> {
    fn log<T>(result: anyhow::Result<Vec<T>>) -> Vec<T> {
        result.unwrap_or_else(|error| {
            tracing::warn!("read the attribute catalog: {error:#}");
            Vec::new()
        })
    }
}

impl Catalog for ReaderCatalog<'_> {
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo> {
        let group = if resource {
            "resource"
        } else {
            signal.as_str()
        };
        Self::log(self.0.catalog_keys(group))
    }

    fn values(&self, signal: Signal, field: &Field) -> Vec<ValueInfo> {
        Self::log(match field {
            Field::Attribute(key) => self.0.catalog_values(signal.as_str(), key),
            Field::Resource(key) => self.0.catalog_values("resource", key),
            Field::Builtin(Builtin::Service) => self.0.column_values("resources", "service"),
            Field::Builtin(Builtin::Name) if signal == Signal::Spans => {
                self.0.catalog_values("span_names", "name")
            }
            Field::Builtin(Builtin::Name) => self.0.column_values("series", "name"),
            Field::Builtin(Builtin::Unit) => self.0.column_values("series", "unit"),
            Field::Builtin(_) => Ok(Vec::new()),
        })
    }
}
