use rusqlite::Connection;

pub enum StoredSchema {
    NotWritten,
    Version(i32),
}

pub fn read_stored_schema(conn: &Connection) -> rusqlite::Result<StoredSchema> {
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
        [],
        |row| row.get(0),
    )?;
    if tables == 0 {
        return Ok(StoredSchema::NotWritten);
    }
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map(StoredSchema::Version)
}
