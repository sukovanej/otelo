//! The attributes with an index, so a query that compares them reads the
//! matching rows and not the whole range.
//!
//! Each indexed attribute is an expression index on
//! `json_extract(attributes, '$."key"')` in every day file. The writer creates
//! and drops them when the set changes, and the query compiler writes the same
//! expression, so SQLite uses the index.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::ensure;
use rusqlite::Connection;
use siner_query::Signal;

use crate::writer::hash;

/// An attribute of the logs or of the spans that has an index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedKey {
    pub signal: Signal,
    pub key: String,
}

impl IndexedKey {
    /// # Errors
    ///
    /// When the signal is not logs or spans, or the key cannot be indexed.
    pub fn new(signal: Signal, key: &str) -> anyhow::Result<Self> {
        ensure!(
            matches!(signal, Signal::Logs | Signal::Spans),
            "only the attributes of logs and spans have indexes"
        );
        ensure!(!key.is_empty(), "the key is empty");
        ensure!(
            !key.contains('"'),
            "a key with a double quote cannot be indexed"
        );
        Ok(Self {
            signal,
            key: key.to_owned(),
        })
    }

    const fn table(&self) -> &'static str {
        match self.signal {
            Signal::Spans => "spans",
            _ => "logs",
        }
    }

    /// The name of the index in a day file.
    fn index_name(&self) -> String {
        format!(
            "attr_{}_{:016x}",
            self.table(),
            hash(&[self.table(), &self.key]).cast_unsigned()
        )
    }
}

/// The set of indexed attributes, shared by the writer, which applies it to
/// the day files, and the query API, which changes it and reads it.
#[derive(Clone, Default)]
pub struct Indexes(Arc<Mutex<(u64, BTreeSet<IndexedKey>)>>);

impl Indexes {
    #[must_use]
    pub fn new(keys: BTreeSet<IndexedKey>) -> Self {
        Self(Arc::new(Mutex::new((0, keys))))
    }

    #[must_use]
    pub fn get(&self) -> BTreeSet<IndexedKey> {
        self.lock().1.clone()
    }

    /// Replaces the set. The writer applies it within a second.
    pub fn set(&self, keys: BTreeSet<IndexedKey>) {
        let mut inner = self.lock();
        inner.0 += 1;
        inner.1 = keys;
    }

    /// A number that changes each time the set does, and the set.
    pub(crate) fn snapshot(&self) -> (u64, BTreeSet<IndexedKey>) {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (u64, BTreeSet<IndexedKey>)> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The SQL of the JSON path to `key`. The key has no double quote.
pub fn json_path(key: &str) -> String {
    format!("'$.\"{}\"'", key.replace('\'', "''"))
}

/// Creates the indexes of `keys` in a day file and drops the others.
pub fn apply(conn: &Connection, keys: &BTreeSet<IndexedKey>) -> rusqlite::Result<()> {
    let desired: HashMap<String, &IndexedKey> =
        keys.iter().map(|key| (key.index_name(), key)).collect();
    let existing: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND name GLOB 'attr_*'")?
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    for name in &existing {
        if !desired.contains_key(name) {
            conn.execute_batch(&format!("DROP INDEX \"{name}\""))?;
        }
    }
    for (name, key) in desired {
        if !existing.contains(&name) {
            conn.execute_batch(&format!(
                "CREATE INDEX \"{name}\" ON {} (json_extract(attributes, {}))",
                key.table(),
                json_path(&key.key)
            ))?;
            tracing::debug!(signal = %key.signal, key = %key.key, "indexed an attribute");
        }
    }
    Ok(())
}
