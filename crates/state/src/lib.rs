mod password;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use otelo_query::Signal;
use rusqlite::{Connection, OptionalExtension};

use crate::password::{generate_password, hash_password};

const STATE_SCHEMA: &str = include_str!("schema.sql");

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
        // The write-ahead log and its index are part of the database.
        ["", "-wal", "-shm"]
            .into_iter()
            .map(|suffix| {
                let mut path = self.path.clone().into_os_string();
                path.push(suffix);
                match std::fs::metadata(&path) {
                    Ok(metadata) => Ok(metadata.len()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
                    Err(error) => {
                        Err(error).with_context(|| format!("read the size of {}", path.display()))
                    }
                }
            })
            .sum()
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

    pub fn has_password(&self) -> anyhow::Result<bool> {
        Ok(self.read_password_hash()?.is_some())
    }

    pub fn replace_password(&self) -> anyhow::Result<String> {
        let password = generate_password()?;
        self.open_connection()?.execute(
            "INSERT INTO passwords (id, hash) VALUES (1, ?1)
             ON CONFLICT (id) DO UPDATE SET hash = excluded.hash",
            [hash_password(&password)],
        )?;
        Ok(password)
    }

    pub fn is_password(&self, candidate: &str) -> anyhow::Result<bool> {
        let Some(hash) = self.read_password_hash()? else {
            bail!("otelo has no password; `otelo init` makes one");
        };
        Ok(hash == hash_password(candidate))
    }

    fn read_password_hash(&self) -> anyhow::Result<Option<[u8; 32]>> {
        Ok(self
            .open_connection()?
            .query_row("SELECT hash FROM passwords", [], |row| row.get(0))
            .optional()?)
    }
}
