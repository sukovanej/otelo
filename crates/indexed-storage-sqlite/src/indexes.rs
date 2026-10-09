use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use rusqlite::Connection;

use crate::attribute_store::find_or_insert_key_id;
use crate::series::hash_fields;

const fn table_name_of(attribute: &IndexedAttribute) -> &'static str {
    match attribute.signal() {
        IndexedSignal::Logs => "logs",
        IndexedSignal::Spans => "spans",
    }
}

#[derive(Clone, Copy)]
pub enum IndexedColumn {
    InternedAttributes,
    LiteralAttributes,
}

impl IndexedColumn {
    const ALL: [Self; 2] = [Self::InternedAttributes, Self::LiteralAttributes];

    pub const fn name(self) -> &'static str {
        match self {
            Self::InternedAttributes => "interned_attributes",
            Self::LiteralAttributes => "literal_attributes",
        }
    }

    const fn index_name_part(self) -> &'static str {
        match self {
            Self::InternedAttributes => "interned_attribute",
            Self::LiteralAttributes => "literal_attribute",
        }
    }
}

fn index_name(attribute: &IndexedAttribute, column: IndexedColumn) -> String {
    format!(
        "{}_{}_{:016x}",
        table_name_of(attribute),
        column.index_name_part(),
        hash_fields(&[table_name_of(attribute), attribute.key()]).cast_unsigned()
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VersionedAttributes {
    pub version: u64,
    pub attributes: BTreeSet<IndexedAttribute>,
}

#[derive(Clone, Default)]
pub struct Indexes(Arc<Mutex<VersionedAttributes>>);

impl Indexes {
    #[must_use]
    pub fn new(attributes: BTreeSet<IndexedAttribute>) -> Self {
        Self(Arc::new(Mutex::new(VersionedAttributes {
            version: 0,
            attributes,
        })))
    }

    #[must_use]
    pub fn attributes(&self) -> BTreeSet<IndexedAttribute> {
        self.lock_attributes().attributes.clone()
    }

    pub fn replace_attributes(&self, attributes: BTreeSet<IndexedAttribute>) {
        let mut versioned = self.lock_attributes();
        versioned.version += 1;
        versioned.attributes = attributes;
    }

    pub(crate) fn versioned_attributes(&self) -> VersionedAttributes {
        self.lock_attributes().clone()
    }

    fn lock_attributes(&self) -> MutexGuard<'_, VersionedAttributes> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn attribute_json_path(key: &str) -> String {
    format!("'$.\"{}\"'", key.replace('\'', "''"))
}

pub fn apply_indexes_to_telemetry_file(
    connection: &Connection,
    attributes: &BTreeSet<IndexedAttribute>,
) -> anyhow::Result<()> {
    let mut wanted_indexes: HashMap<String, (&IndexedAttribute, IndexedColumn)> = HashMap::new();
    for attribute in attributes {
        for column in IndexedColumn::ALL {
            wanted_indexes.insert(index_name(attribute, column), (attribute, column));
        }
    }
    let existing_index_names: Vec<String> = connection
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'index'
               AND (name GLOB 'logs_interned_attribute_*' OR name GLOB 'logs_literal_attribute_*'
                    OR name GLOB 'spans_interned_attribute_*'
                    OR name GLOB 'spans_literal_attribute_*')",
        )?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for name in &existing_index_names {
        if !wanted_indexes.contains_key(name) {
            connection.execute_batch(&format!("DROP INDEX \"{name}\""))?;
        }
    }
    let mut key_ids = HashMap::new();
    for (name, (attribute, column)) in wanted_indexes {
        if existing_index_names.contains(&name) {
            continue;
        }
        let key_id = find_or_insert_key_id(connection, &mut key_ids, attribute.key())?;
        let extracted = format!("json_extract({}, {})", column.name(), key_id.json_path());
        connection.execute_batch(&format!(
            "CREATE INDEX \"{name}\" ON {} ({extracted}) WHERE {extracted} IS NOT NULL",
            table_name_of(attribute),
        ))?;
        tracing::debug!(signal = %attribute.signal(), key = attribute.key(), column = column.name(), "indexed an attribute");
    }
    Ok(())
}
