mod password;
mod session;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail, ensure};
use otelo_indexed_storage::{IndexedAttribute, IndexedSignal, now_unix_nanos};
use otelo_query::Signal;
use rusqlite::{Connection, OptionalExtension};

use crate::password::{generate_password, hash_password, is_password_of_hash};

pub use session::SessionToken;

const MIGRATIONS: [&str; 2] = [
    include_str!("migrations/1_telemetry_indexes.sql"),
    include_str!("migrations/2_logins.sql"),
];

const SESSION_IDLE_LIMIT_NS: i64 = 30 * 24 * 3600 * 1_000_000_000;

// The API renews a session on each request, and requests come in parallel.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    pub fn open(data_directory: &Path) -> anyhow::Result<Self> {
        let state = Self {
            path: data_directory.join("state.sqlite"),
        };
        let mut connection = state.open_connection()?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .with_context(|| format!("turn on the write-ahead log of {}", state.path.display()))?;
        apply_migrations(&mut connection)
            .with_context(|| format!("migrate {}", state.path.display()))?;
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
        let connection = Connection::open(&self.path)
            .with_context(|| format!("open {}", self.path.display()))?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        Ok(connection)
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
        let hash = hash_password(&password)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO passwords (id, hash) VALUES (1, ?1)
             ON CONFLICT (id) DO UPDATE SET hash = excluded.hash",
            [&hash],
        )?;
        transaction.execute("DELETE FROM sessions", [])?;
        transaction.commit()?;
        Ok(password)
    }

    pub fn is_password(&self, candidate: &str) -> anyhow::Result<bool> {
        let Some(hash) = self.read_password_hash()? else {
            bail!("otelo has no password; `otelo init` makes one");
        };
        is_password_of_hash(candidate, &hash)
    }

    fn read_password_hash(&self) -> anyhow::Result<Option<String>> {
        Ok(self
            .open_connection()?
            .query_row("SELECT hash FROM passwords", [], |row| row.get(0))
            .optional()?)
    }

    pub fn start_session(&self) -> anyhow::Result<SessionToken> {
        let token = SessionToken::generate()?;
        let now = now_unix_nanos();
        let connection = self.open_connection()?;
        connection.execute(
            "DELETE FROM sessions WHERE last_used_at <= ?1",
            [now - SESSION_IDLE_LIMIT_NS],
        )?;
        connection.execute(
            "INSERT INTO sessions (token_hash, created_at, last_used_at) VALUES (?1, ?2, ?2)",
            (token.hash(), now),
        )?;
        Ok(token)
    }

    pub fn renew_session(&self, token: &SessionToken) -> anyhow::Result<bool> {
        let now = now_unix_nanos();
        let renewed = self.open_connection()?.execute(
            "UPDATE sessions SET last_used_at = ?2 WHERE token_hash = ?1 AND last_used_at > ?3",
            (token.hash(), now, now - SESSION_IDLE_LIMIT_NS),
        )?;
        Ok(renewed > 0)
    }

    pub fn end_session(&self, token: &SessionToken) -> anyhow::Result<()> {
        self.open_connection()?
            .execute("DELETE FROM sessions WHERE token_hash = ?1", [token.hash()])?;
        Ok(())
    }
}

fn apply_migrations(connection: &mut Connection) -> anyhow::Result<()> {
    let applied_steps: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let numbered_steps = (1_u32..).zip(MIGRATIONS);
    let known_steps = numbered_steps
        .clone()
        .last()
        .map_or(0, |(number, _)| number);
    ensure!(
        applied_steps <= known_steps,
        "a newer otelo wrote it: it has {applied_steps} steps of the schema, and this otelo knows {known_steps}"
    );
    for (step_number, step) in numbered_steps.skip_while(|(number, _)| *number <= applied_steps) {
        let transaction = connection.transaction()?;
        transaction
            .execute_batch(step)
            .with_context(|| format!("apply step {step_number}"))?;
        transaction.pragma_update(None, "user_version", step_number)?;
        transaction.commit()?;
    }
    Ok(())
}
