//! The state file, `state.sqlite` in the data directory. For now it holds the
//! attributes to index.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use rusqlite::Connection;
use siner_query::Signal;
use siner_telemetry::IndexedKey;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS telemetry_indexes (
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  PRIMARY KEY (signal, key)
) WITHOUT ROWID;
";

pub struct State {
    path: PathBuf,
}

impl State {
    /// Opens the state file in `data`, and makes it when it is missing.
    pub fn open(data: &Path) -> anyhow::Result<Self> {
        let state = Self {
            path: data.join("state.sqlite"),
        };
        state
            .conn()?
            .execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", state.path.display()))?;
        Ok(state)
    }

    fn conn(&self) -> anyhow::Result<Connection> {
        Connection::open(&self.path).with_context(|| format!("open {}", self.path.display()))
    }

    /// The attributes to index.
    pub fn indexes(&self) -> anyhow::Result<BTreeSet<IndexedKey>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT signal, key FROM telemetry_indexes")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut keys = BTreeSet::new();
        for row in rows {
            let (signal, key) = row?;
            let signal: Signal = signal.parse().map_err(anyhow::Error::msg)?;
            keys.insert(IndexedKey::new(signal, &key)?);
        }
        Ok(keys)
    }

    pub fn add_index(&self, key: &IndexedKey) -> anyhow::Result<()> {
        self.conn()?.execute(
            "INSERT INTO telemetry_indexes (signal, key) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            (key.signal.as_str(), &key.key),
        )?;
        Ok(())
    }

    /// Whether the key had an index.
    pub fn remove_index(&self, key: &IndexedKey) -> anyhow::Result<bool> {
        let removed = self.conn()?.execute(
            "DELETE FROM telemetry_indexes WHERE signal = ?1 AND key = ?2",
            (key.signal.as_str(), &key.key),
        )?;
        Ok(removed > 0)
    }
}
