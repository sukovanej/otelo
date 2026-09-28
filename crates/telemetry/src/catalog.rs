//! The attribute keys and values of the records in a day file, which the
//! writer keeps in `attribute_keys` and `attribute_values` for completion.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, Transaction, params};
use serde_json::{Map, Value};

/// The most values a key keeps. A key with more, such as a user ID, keeps the
/// first ones and is marked as having many values.
pub const VALUES_PER_KEY: usize = 200;

/// The longest value the catalog keeps. A longer one is no use to complete.
const MAX_VALUE_LEN: usize = 100;

/// The groups of keys in the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Logs,
    Spans,
    /// The labels of metric series.
    Metrics,
    Resource,
    /// The names of spans, under the key `name`.
    SpanNames,
}

impl Group {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logs => "logs",
            Self::Spans => "spans",
            Self::Metrics => "metrics",
            Self::Resource => "resource",
            Self::SpanNames => "span_names",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [
            Self::Logs,
            Self::Spans,
            Self::Metrics,
            Self::Resource,
            Self::SpanNames,
        ]
        .into_iter()
        .find(|group| group.as_str() == text)
    }
}

/// The JSON type of a value, as the catalog names it.
pub fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) if n.is_f64() => "float",
        Value::Number(_) => "int",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// What the catalog of one day file already holds, so that a write only
/// touches the rows of new keys and values, and counts.
#[derive(Default)]
pub struct Catalog {
    keys: HashMap<(Group, String), Known>,
}

struct Known {
    kind: &'static str,
    values: HashSet<String>,
    many: bool,
}

/// The changes one transaction makes to the catalog.
#[derive(Default)]
pub struct Delta {
    keys: HashMap<(Group, String), (&'static str, i64)>,
    values: HashMap<(Group, String, String), i64>,
}

impl Catalog {
    /// Reads what the catalog tables of a day file hold.
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut catalog = Self::default();
        let mut stmt = conn.prepare("SELECT signal, key, type, many_values FROM attribute_keys")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = Group::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let kind = row.get::<_, String>(2)?;
            catalog.keys.insert(
                (group, row.get(1)?),
                Known {
                    kind: static_type(&kind),
                    values: HashSet::new(),
                    many: row.get(3)?,
                },
            );
        }
        let mut stmt = conn.prepare("SELECT signal, key, value FROM attribute_values")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = Group::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let key: String = row.get(1)?;
            let known = catalog.keys.entry((group, key)).or_insert_with(|| Known {
                kind: "string",
                values: HashSet::new(),
                many: false,
            });
            known.values.insert(row.get(2)?);
        }
        Ok(catalog)
    }

    /// Counts every attribute of one record.
    pub fn record(&mut self, delta: &mut Delta, group: Group, attributes: &Map<String, Value>) {
        for (key, value) in attributes {
            self.value(delta, group, key, value);
        }
    }

    /// Counts one value of `key`.
    pub fn value(&mut self, delta: &mut Delta, group: Group, key: &str, value: &Value) {
        let kind = type_name(value);
        let slot = (group, key.to_owned());
        let known = self.keys.entry(slot.clone()).or_insert_with(|| Known {
            kind,
            values: HashSet::new(),
            many: false,
        });
        if known.kind != kind {
            known.kind = "mixed";
        }
        let entry = delta.keys.entry(slot).or_insert((known.kind, 0));
        entry.0 = known.kind;
        entry.1 += 1;
        let Some(text) = value_text(value) else {
            return;
        };
        if known.values.contains(&text) {
            *delta
                .values
                .entry((group, key.to_owned(), text))
                .or_default() += 1;
        } else if known.values.len() < VALUES_PER_KEY {
            known.values.insert(text.clone());
            *delta
                .values
                .entry((group, key.to_owned(), text))
                .or_default() += 1;
        } else {
            known.many = true;
        }
    }

    /// Writes `delta` in the transaction.
    pub fn flush(&self, tx: &Transaction, delta: Delta) -> rusqlite::Result<()> {
        let mut insert = tx.prepare_cached(
            "INSERT INTO attribute_keys (signal, key, type, count, many_values)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (signal, key) DO UPDATE SET
               count = count + excluded.count,
               type = CASE WHEN type = excluded.type THEN type ELSE 'mixed' END,
               many_values = max(many_values, excluded.many_values)",
        )?;
        for ((group, key), (kind, count)) in delta.keys {
            let many = self.keys.get(&(group, key.clone())).is_some_and(|k| k.many);
            insert.execute(params![group.as_str(), key, kind, count, many])?;
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO attribute_values (signal, key, value, count) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (signal, key, value) DO UPDATE SET count = count + excluded.count",
        )?;
        for ((group, key, value), count) in delta.values {
            insert.execute(params![group.as_str(), key, value, count])?;
        }
        Ok(())
    }
}

/// A scalar value as the JSON the catalog keeps, or `None` when it is not
/// worth completing.
fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if text.len() > MAX_VALUE_LEN => None,
        Value::String(_) | Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

fn static_type(kind: &str) -> &'static str {
    ["null", "bool", "float", "int", "string", "array", "object"]
        .into_iter()
        .find(|name| *name == kind)
        .unwrap_or("mixed")
}
