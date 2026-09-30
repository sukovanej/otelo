use std::path::Path;
use std::{fs, io};

use rusqlite::Connection;

pub enum StoredSchema {
    NotWritten,
    Version(i32),
}

pub fn read_stored_schema(connection: &Connection) -> rusqlite::Result<StoredSchema> {
    let table_count: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
        [],
        |row| row.get(0),
    )?;
    if table_count == 0 {
        return Ok(StoredSchema::NotWritten);
    }
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map(StoredSchema::Version)
}

pub fn set_aside_file_of_another_schema(path: &Path, schema_version: i32) -> anyhow::Result<()> {
    let connection = Connection::open(path)?;
    let version = match read_stored_schema(&connection)? {
        StoredSchema::NotWritten => return Ok(()),
        StoredSchema::Version(version) if version == schema_version => return Ok(()),
        StoredSchema::Version(version) => version,
    };
    // The file has to hold all its rows before it moves without its write-ahead log.
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    drop(connection);
    let set_aside_path = path.with_extension(format!("sqlite.schema-{version}"));
    fs::rename(path, &set_aside_path)?;
    // A write-ahead log left behind would be read as the log of the next file of that name.
    for suffix in ["sqlite-wal", "sqlite-shm"] {
        match fs::remove_file(path.with_extension(suffix)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
    }
    tracing::warn!(
        file = %set_aside_path.display(),
        "set aside a file of schema version {version}; this otelo reads version {schema_version}"
    );
    Ok(())
}
