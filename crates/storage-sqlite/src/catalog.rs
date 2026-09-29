use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, Transaction, params};

use siner_storage::{AttributeValue, Attributes};

pub const MAX_VALUES_PER_KEY: usize = 200;

// A longer value is no use to complete.
const MAX_VALUE_LEN: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyGroup {
    Logs,
    Spans,
    Metrics,
    Resource,
    SpanNames,
}

impl KeyGroup {
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

#[derive(Default)]
pub struct CatalogCache {
    keys: HashMap<(KeyGroup, String), KnownKey>,
}

struct KnownKey {
    kind: &'static str,
    values: HashSet<String>,
    many_values: bool,
}

#[derive(Default)]
pub struct CatalogDelta {
    keys: HashMap<(KeyGroup, String), (&'static str, i64)>,
    values: HashMap<(KeyGroup, String, String), i64>,
}

impl CatalogCache {
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut catalog = Self::default();
        let mut stmt = conn.prepare("SELECT signal, key, type, many_values FROM attribute_keys")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = KeyGroup::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let kind = row.get::<_, String>(2)?;
            catalog.keys.insert(
                (group, row.get(1)?),
                KnownKey {
                    kind: static_type(&kind),
                    values: HashSet::new(),
                    many_values: row.get(3)?,
                },
            );
        }
        let mut stmt = conn.prepare("SELECT signal, key, value FROM attribute_values")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = KeyGroup::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let key: String = row.get(1)?;
            let known = catalog
                .keys
                .entry((group, key))
                .or_insert_with(|| KnownKey {
                    kind: "string",
                    values: HashSet::new(),
                    many_values: false,
                });
            known.values.insert(row.get(2)?);
        }
        Ok(catalog)
    }

    pub fn count_attributes(
        &mut self,
        delta: &mut CatalogDelta,
        group: KeyGroup,
        attributes: &Attributes,
    ) {
        for (key, value) in attributes {
            self.count_value(delta, group, key, value);
        }
    }

    pub fn count_value(
        &mut self,
        delta: &mut CatalogDelta,
        group: KeyGroup,
        key: &str,
        value: &AttributeValue,
    ) {
        let kind = value.type_name();
        let slot = (group, key.to_owned());
        let known = self.keys.entry(slot.clone()).or_insert_with(|| KnownKey {
            kind,
            values: HashSet::new(),
            many_values: false,
        });
        if known.kind != kind {
            known.kind = "mixed";
        }
        let entry = delta.keys.entry(slot).or_insert((known.kind, 0));
        entry.0 = known.kind;
        entry.1 += 1;
        let Some(text) = completable_value_json(value) else {
            return;
        };
        if known.values.contains(&text) {
            *delta
                .values
                .entry((group, key.to_owned(), text))
                .or_default() += 1;
        } else if known.values.len() < MAX_VALUES_PER_KEY {
            known.values.insert(text.clone());
            *delta
                .values
                .entry((group, key.to_owned(), text))
                .or_default() += 1;
        } else {
            known.many_values = true;
        }
    }

    pub fn write_delta(&self, tx: &Transaction, delta: CatalogDelta) -> rusqlite::Result<()> {
        let mut insert = tx.prepare_cached(
            "INSERT INTO attribute_keys (signal, key, type, count, many_values)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (signal, key) DO UPDATE SET
               count = count + excluded.count,
               type = CASE WHEN type = excluded.type THEN type ELSE 'mixed' END,
               many_values = max(many_values, excluded.many_values)",
        )?;
        for ((group, key), (kind, count)) in delta.keys {
            let many_values = self
                .keys
                .get(&(group, key.clone()))
                .is_some_and(|k| k.many_values);
            insert.execute(params![group.as_str(), key, kind, count, many_values])?;
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

fn completable_value_json(value: &AttributeValue) -> Option<String> {
    match value {
        AttributeValue::String(text) if text.len() > MAX_VALUE_LEN => None,
        AttributeValue::String(_)
        | AttributeValue::Int(_)
        | AttributeValue::Double(_)
        | AttributeValue::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

fn static_type(kind: &str) -> &'static str {
    ["null", "bool", "float", "int", "string", "array", "object"]
        .into_iter()
        .find(|name| *name == kind)
        .unwrap_or("mixed")
}
