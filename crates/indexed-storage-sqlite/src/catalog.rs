use std::collections::{HashMap, HashSet};

use otelo_query::{Signal, ValueType};
use rusqlite::{Connection, Transaction, params};

use otelo_indexed_storage::{AttributeValue, Attributes};

use crate::day::Day;

pub const MAX_VALUES_PER_KEY: usize = 200;

const MAX_COMPLETABLE_VALUE_BYTES: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttributeOwner {
    Log,
    Span,
    MetricSeries,
    Resource,
}

impl AttributeOwner {
    const ALL: [Self; 4] = [Self::Log, Self::Span, Self::MetricSeries, Self::Resource];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Span => "span",
            Self::MetricSeries => "metric_series",
            Self::Resource => "resource",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|owner| owner.name() == text)
    }
}

impl From<Signal> for AttributeOwner {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Logs => Self::Log,
            Signal::Spans => Self::Span,
            Signal::Metrics => Self::MetricSeries,
        }
    }
}

pub fn json_type_from_stored_name(name: &str) -> ValueType {
    ValueType::from_name(name).unwrap_or(ValueType::Mixed)
}

type OwnedKey = (Day, AttributeOwner, String);

#[derive(Default)]
pub struct CatalogCache {
    loaded_days: HashSet<Day>,
    keys: HashMap<OwnedKey, KnownKey>,
    span_names_by_day: HashMap<Day, HashSet<String>>,
}

struct KnownKey {
    json_type: ValueType,
    values: HashSet<String>,
    has_more_values_than_listed: bool,
}

struct KeyDelta {
    json_type: ValueType,
    record_count: i64,
}

#[derive(Default)]
pub struct CatalogDelta {
    keys: HashMap<OwnedKey, KeyDelta>,
    values: HashMap<(Day, AttributeOwner, String, String), i64>,
    span_names: HashMap<(Day, String), i64>,
}

impl CatalogCache {
    pub fn load_day(&mut self, connection: &Connection, day: Day) -> rusqlite::Result<()> {
        if !self.loaded_days.insert(day) {
            return Ok(());
        }
        let mut keys_statement = connection.prepare_cached(
            "SELECT attribute_owner, key, json_type, has_more_values_than_listed
             FROM attribute_key_counts
             WHERE day = ?1",
        )?;
        let mut rows = keys_statement.query([day])?;
        while let Some(row) = rows.next()? {
            let Some(owner) = AttributeOwner::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            self.keys.insert(
                (day, owner, row.get(1)?),
                KnownKey {
                    json_type: json_type_from_stored_name(&row.get::<_, String>(2)?),
                    values: HashSet::new(),
                    has_more_values_than_listed: row.get(3)?,
                },
            );
        }
        let mut values_statement = connection.prepare_cached(
            "SELECT attribute_owner, key, value FROM attribute_value_counts WHERE day = ?1",
        )?;
        let mut rows = values_statement.query([day])?;
        while let Some(row) = rows.next()? {
            let Some(owner) = AttributeOwner::parse(&row.get::<_, String>(0)?) else {
                continue;
            };
            let known = self
                .keys
                .entry((day, owner, row.get(1)?))
                .or_insert_with(|| KnownKey {
                    json_type: ValueType::String,
                    values: HashSet::new(),
                    has_more_values_than_listed: false,
                });
            known.values.insert(row.get(2)?);
        }
        let mut span_names_statement =
            connection.prepare_cached("SELECT name FROM span_name_counts WHERE day = ?1")?;
        let span_names = span_names_statement
            .query_map([day], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        self.span_names_by_day.insert(day, span_names);
        Ok(())
    }

    pub fn forget_days_before(&mut self, oldest_kept_day: Day) {
        self.loaded_days.retain(|&day| day >= oldest_kept_day);
        self.keys.retain(|(day, _, _), _| *day >= oldest_kept_day);
        self.span_names_by_day
            .retain(|&day, _| day >= oldest_kept_day);
    }

    pub fn count_attributes(
        &mut self,
        delta: &mut CatalogDelta,
        day: Day,
        owner: AttributeOwner,
        attributes: &Attributes,
    ) {
        for (key, value) in attributes {
            self.count_value(delta, day, owner, key, value);
        }
    }

    fn count_value(
        &mut self,
        delta: &mut CatalogDelta,
        day: Day,
        owner: AttributeOwner,
        key: &str,
        value: &AttributeValue,
    ) {
        let json_type = value.value_type();
        let owned_key = (day, owner, key.to_owned());
        let known = self
            .keys
            .entry(owned_key.clone())
            .or_insert_with(|| KnownKey {
                json_type,
                values: HashSet::new(),
                has_more_values_than_listed: false,
            });
        if known.json_type != json_type {
            known.json_type = ValueType::Mixed;
        }
        let key_delta = delta.keys.entry(owned_key).or_insert(KeyDelta {
            json_type: known.json_type,
            record_count: 0,
        });
        key_delta.json_type = known.json_type;
        key_delta.record_count += 1;
        let Some(value_json) = completable_value_json(value) else {
            // A string too long to list is still one of the values of the key.
            known.has_more_values_than_listed |= matches!(value, AttributeValue::String(_));
            return;
        };
        if known.values.contains(&value_json) || known.values.len() < MAX_VALUES_PER_KEY {
            known.values.insert(value_json.clone());
            *delta
                .values
                .entry((day, owner, key.to_owned(), value_json))
                .or_default() += 1;
        } else {
            known.has_more_values_than_listed = true;
        }
    }

    pub fn count_span_name(&mut self, delta: &mut CatalogDelta, day: Day, name: &str) {
        let known_names = self.span_names_by_day.entry(day).or_default();
        if known_names.contains(name) || known_names.len() < MAX_VALUES_PER_KEY {
            known_names.insert(name.to_owned());
            *delta.span_names.entry((day, name.to_owned())).or_default() += 1;
        }
    }

    pub fn write_delta(
        &self,
        transaction: &Transaction,
        delta: CatalogDelta,
    ) -> rusqlite::Result<()> {
        let mut insert_key = transaction.prepare_cached(
            "INSERT INTO attribute_key_counts
               (day, attribute_owner, key, json_type, record_count, has_more_values_than_listed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (day, attribute_owner, key) DO UPDATE SET
               record_count = record_count + excluded.record_count,
               json_type = CASE
                 WHEN json_type = excluded.json_type THEN json_type
                 ELSE 'mixed'
               END,
               has_more_values_than_listed =
                 max(has_more_values_than_listed, excluded.has_more_values_than_listed)",
        )?;
        for (
            owned_key,
            KeyDelta {
                json_type,
                record_count,
            },
        ) in delta.keys
        {
            let has_more_values_than_listed = self
                .keys
                .get(&owned_key)
                .is_some_and(|known| known.has_more_values_than_listed);
            let (day, owner, key) = owned_key;
            insert_key.execute(params![
                day,
                owner.name(),
                key,
                json_type.name(),
                record_count,
                has_more_values_than_listed
            ])?;
        }
        let mut insert_value = transaction.prepare_cached(
            "INSERT INTO attribute_value_counts (day, attribute_owner, key, value, record_count)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (day, attribute_owner, key, value) DO UPDATE
             SET record_count = record_count + excluded.record_count",
        )?;
        for ((day, owner, key, value), record_count) in delta.values {
            insert_value.execute(params![day, owner.name(), key, value, record_count])?;
        }
        let mut insert_span_name = transaction.prepare_cached(
            "INSERT INTO span_name_counts (day, name, record_count) VALUES (?1, ?2, ?3)
             ON CONFLICT (day, name) DO UPDATE
             SET record_count = record_count + excluded.record_count",
        )?;
        for ((day, name), record_count) in delta.span_names {
            insert_span_name.execute(params![day, name, record_count])?;
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
