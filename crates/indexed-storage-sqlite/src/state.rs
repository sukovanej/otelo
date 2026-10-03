use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use otelo_query::Signal;
use rusqlite::Connection;

use crate::sqlite::size_of_database_in_bytes;

const STATE_SCHEMA: &str = "
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS telemetry_indexes (
  signal TEXT NOT NULL,
  key TEXT NOT NULL,
  PRIMARY KEY (signal, key)
) WITHOUT ROWID;
";

pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    pub fn open(data_directory: &Path) -> anyhow::Result<Self> {
        let state = Self {
            path: data_directory.join("state.sqlite"),
        };
        state
            .open_connection()?
            .execute_batch(STATE_SCHEMA)
            .with_context(|| format!("create the schema in {}", state.path.display()))?;
        Ok(state)
    }

    pub fn size_in_bytes(&self) -> anyhow::Result<u64> {
        size_of_database_in_bytes(&self.path)
    }

    fn open_connection(&self) -> anyhow::Result<Connection> {
        Connection::open(&self.path).with_context(|| format!("open {}", self.path.display()))
    }

    pub fn indexed_attributes(&self) -> anyhow::Result<BTreeSet<IndexedAttribute>> {
        let connection = self.open_connection()?;
        let mut statement = connection.prepare("SELECT signal, key FROM telemetry_indexes")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut attributes = BTreeSet::new();
        for row in rows {
            let (signal, key) = row?;
            let signal: Signal = signal.parse().map_err(anyhow::Error::msg)?;
            attributes.insert(IndexedAttribute::new(
                IndexedSignal::try_from(signal)?,
                &key,
            )?);
        }
        Ok(attributes)
    }

    pub fn add_indexed_attribute(&self, attribute: &IndexedAttribute) -> anyhow::Result<()> {
        self.open_connection()?.execute(
            "INSERT INTO telemetry_indexes (signal, key) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            (Signal::from(attribute.signal()).name(), attribute.key()),
        )?;
        Ok(())
    }

    pub fn remove_indexed_attribute(&self, attribute: &IndexedAttribute) -> anyhow::Result<bool> {
        let removed = self.open_connection()?.execute(
            "DELETE FROM telemetry_indexes WHERE signal = ?1 AND key = ?2",
            (Signal::from(attribute.signal()).name(), attribute.key()),
        )?;
        Ok(removed > 0)
    }
}
