use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use otelo_query::Signal;
use otelo_storage::{IndexedAttribute, IndexedSignal};
use rusqlite::Connection;

const SCHEMA: &str = "
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
    pub fn open(data: &Path) -> anyhow::Result<Self> {
        let state = Self {
            path: data.join("state.sqlite"),
        };
        state
            .connect()?
            .execute_batch(SCHEMA)
            .with_context(|| format!("create the schema in {}", state.path.display()))?;
        Ok(state)
    }

    fn connect(&self) -> anyhow::Result<Connection> {
        Connection::open(&self.path).with_context(|| format!("open {}", self.path.display()))
    }

    pub fn indexed_attributes(&self) -> anyhow::Result<BTreeSet<IndexedAttribute>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT signal, key FROM telemetry_indexes")?;
        let rows = stmt.query_map([], |row| {
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
        self.connect()?.execute(
            "INSERT INTO telemetry_indexes (signal, key) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
            (attribute.signal().signal().as_str(), attribute.key()),
        )?;
        Ok(())
    }

    pub fn remove_indexed_attribute(&self, attribute: &IndexedAttribute) -> anyhow::Result<bool> {
        let removed = self.connect()?.execute(
            "DELETE FROM telemetry_indexes WHERE signal = ?1 AND key = ?2",
            (attribute.signal().signal().as_str(), attribute.key()),
        )?;
        Ok(removed > 0)
    }
}
