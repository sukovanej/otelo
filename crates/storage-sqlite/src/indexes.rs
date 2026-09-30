use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use otelo_storage::{IndexedAttribute, IndexedSignal};
use rusqlite::Connection;

use crate::series::hash_fields;

const fn table(attribute: &IndexedAttribute) -> &'static str {
    match attribute.signal() {
        IndexedSignal::Logs => "logs",
        IndexedSignal::Spans => "spans",
    }
}

fn index_name(attribute: &IndexedAttribute) -> String {
    format!(
        "attr_{}_{:016x}",
        table(attribute),
        hash_fields(&[table(attribute), attribute.key()]).cast_unsigned()
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
        self.lock().attributes.clone()
    }

    pub fn replace_attributes(&self, attributes: BTreeSet<IndexedAttribute>) {
        let mut versioned = self.lock();
        versioned.version += 1;
        versioned.attributes = attributes;
    }

    pub(crate) fn versioned_attributes(&self) -> VersionedAttributes {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, VersionedAttributes> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn attribute_json_path(key: &str) -> String {
    format!("'$.\"{}\"'", key.replace('\'', "''"))
}

pub fn apply_indexes_to_day_file(
    conn: &Connection,
    attributes: &BTreeSet<IndexedAttribute>,
) -> rusqlite::Result<()> {
    let desired: HashMap<String, &IndexedAttribute> = attributes
        .iter()
        .map(|attribute| (index_name(attribute), attribute))
        .collect();
    let existing: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name GLOB 'attr_*'")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for name in &existing {
        if !desired.contains_key(name) {
            conn.execute_batch(&format!("DROP INDEX \"{name}\""))?;
        }
    }
    for (name, attribute) in desired {
        if !existing.contains(&name) {
            conn.execute_batch(&format!(
                "CREATE INDEX \"{name}\" ON {} (json_extract(attributes, {}))",
                table(attribute),
                attribute_json_path(attribute.key())
            ))?;
            tracing::debug!(signal = %attribute.signal(), key = attribute.key(), "indexed an attribute");
        }
    }
    Ok(())
}
