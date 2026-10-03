use std::collections::{HashMap, HashSet};

use otelo_query::{Signal, ValueType};
use rusqlite::{Connection, Transaction, params};

use otelo_indexed_storage::{AttributeValue, Attributes};

pub const MAX_VALUES_PER_KEY: usize = 200;

const MAX_COMPLETABLE_VALUE_BYTES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyGroup {
    Logs,
    Spans,
    Metrics,
    Resource,
    SpanNames,
}

impl KeyGroup {
    pub const fn name(self) -> &'static str {
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
        .find(|group| group.name() == text)
    }
}

impl From<Signal> for KeyGroup {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Logs => Self::Logs,
            Signal::Spans => Self::Spans,
            Signal::Metrics => Self::Metrics,
        }
    }
}

pub fn value_type_from_stored_name(name: &str) -> ValueType {
    ValueType::from_name(name).unwrap_or(ValueType::Mixed)
}

#[derive(Default)]
pub struct CatalogCache {
    keys: HashMap<(KeyGroup, String), KnownKey>,
}

struct KnownKey {
    value_type: ValueType,
    values: HashSet<String>,
    has_more_values_than_listed: bool,
}

struct KeyDelta {
    value_type: ValueType,
    count: i64,
}

#[derive(Default)]
pub struct CatalogDelta {
    keys: HashMap<(KeyGroup, String), KeyDelta>,
    values: HashMap<(KeyGroup, String, String), i64>,
}

impl CatalogCache {
    pub fn load(connection: &Connection) -> rusqlite::Result<Self> {
        let mut catalog = Self::default();
        let mut keys_statement = connection.prepare(
            "SELECT key_group, key, value_type, has_more_values_than_listed FROM attribute_keys",
        )?;
        let mut rows = keys_statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = KeyGroup::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            catalog.keys.insert(
                (group, row.get(1)?),
                KnownKey {
                    value_type: value_type_from_stored_name(&row.get::<_, String>(2)?),
                    values: HashSet::new(),
                    has_more_values_than_listed: row.get(3)?,
                },
            );
        }
        let mut values_statement =
            connection.prepare("SELECT key_group, key, value FROM attribute_values")?;
        let mut rows = values_statement.query([])?;
        while let Some(row) = rows.next()? {
            let Some(group) = KeyGroup::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let key: String = row.get(1)?;
            let known = catalog
                .keys
                .entry((group, key))
                .or_insert_with(|| KnownKey {
                    value_type: ValueType::String,
                    values: HashSet::new(),
                    has_more_values_than_listed: false,
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
        let value_type = value.value_type();
        let group_and_key = (group, key.to_owned());
        let known = self
            .keys
            .entry(group_and_key.clone())
            .or_insert_with(|| KnownKey {
                value_type,
                values: HashSet::new(),
                has_more_values_than_listed: false,
            });
        if known.value_type != value_type {
            known.value_type = ValueType::Mixed;
        }
        let key_delta = delta.keys.entry(group_and_key).or_insert(KeyDelta {
            value_type: known.value_type,
            count: 0,
        });
        key_delta.value_type = known.value_type;
        key_delta.count += 1;
        let Some(value_json) = completable_value_json(value) else {
            // A string too long to list is still one of the values of the key.
            known.has_more_values_than_listed |= matches!(value, AttributeValue::String(_));
            return;
        };
        if known.values.contains(&value_json) {
            *delta
                .values
                .entry((group, key.to_owned(), value_json))
                .or_default() += 1;
        } else if known.values.len() < MAX_VALUES_PER_KEY {
            known.values.insert(value_json.clone());
            *delta
                .values
                .entry((group, key.to_owned(), value_json))
                .or_default() += 1;
        } else {
            known.has_more_values_than_listed = true;
        }
    }

    pub fn write_delta(
        &self,
        transaction: &Transaction,
        delta: CatalogDelta,
    ) -> rusqlite::Result<()> {
        let mut insert_key = transaction.prepare_cached(
            "INSERT INTO attribute_keys
               (key_group, key, value_type, count, has_more_values_than_listed)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (key_group, key) DO UPDATE SET
               count = count + excluded.count,
               value_type = CASE
                 WHEN value_type = excluded.value_type THEN value_type
                 ELSE 'mixed'
               END,
               has_more_values_than_listed =
                 max(has_more_values_than_listed, excluded.has_more_values_than_listed)",
        )?;
        for ((group, key), KeyDelta { value_type, count }) in delta.keys {
            let has_more_values_than_listed = self
                .keys
                .get(&(group, key.clone()))
                .is_some_and(|known| known.has_more_values_than_listed);
            insert_key.execute(params![
                group.name(),
                key,
                value_type.name(),
                count,
                has_more_values_than_listed
            ])?;
        }
        let mut insert_value = transaction.prepare_cached(
            "INSERT INTO attribute_values (key_group, key, value, count) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (key_group, key, value) DO UPDATE SET count = count + excluded.count",
        )?;
        for ((group, key, value), count) in delta.values {
            insert_value.execute(params![group.name(), key, value, count])?;
        }
        Ok(())
    }
}

fn completable_value_json(value: &AttributeValue) -> Option<String> {
    match value {
        AttributeValue::String(text) if text.len() > MAX_COMPLETABLE_VALUE_BYTES => None,
        AttributeValue::String(_)
        | AttributeValue::Int(_)
        | AttributeValue::Double(_)
        | AttributeValue::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}
