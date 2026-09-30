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
