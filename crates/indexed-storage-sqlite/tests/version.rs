use otelo_indexed_storage_sqlite::{
    OtherStorageVersion, SCHEMA_HASH_AT_STORAGE_VERSION, STORAGE_VERSION, TELEMETRY_FILE_NAME,
    TelemetryFile,
};
use rusqlite::Connection;
use twox_hash::XxHash3_64;

#[test]
fn the_schema_has_the_hash_of_its_storage_version() {
    let schema_hash = XxHash3_64::oneshot(include_bytes!("../src/schema.sql"));
    assert_eq!(
        schema_hash, SCHEMA_HASH_AT_STORAGE_VERSION,
        "schema.sql changed: bump STORAGE_VERSION and set SCHEMA_HASH_AT_STORAGE_VERSION to \
         {schema_hash:#018x}"
    );
}

#[test]
fn a_new_file_gets_the_storage_version() {
    let directory = tempfile::tempdir().unwrap();
    TelemetryFile::open(directory.path()).unwrap();
    let connection = Connection::open(directory.path().join(TELEMETRY_FILE_NAME)).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, STORAGE_VERSION);
    TelemetryFile::open(directory.path()).unwrap();
}

fn open_with_version(version: Option<i64>) -> anyhow::Error {
    let directory = tempfile::tempdir().unwrap();
    let connection = Connection::open(directory.path().join(TELEMETRY_FILE_NAME)).unwrap();
    connection
        .execute_batch("CREATE TABLE logs (body TEXT NOT NULL)")
        .unwrap();
    if let Some(version) = version {
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
    }
    drop(connection);
    TelemetryFile::open(directory.path()).err().unwrap()
}

#[test]
fn a_file_of_another_version_does_not_open() {
    let error = open_with_version(Some(STORAGE_VERSION + 1));
    assert_eq!(
        error.downcast_ref::<OtherStorageVersion>(),
        Some(&OtherStorageVersion {
            found_version: STORAGE_VERSION + 1
        })
    );
    assert_eq!(
        error.to_string(),
        format!(
            "telemetry.sqlite has storage version {} and this otelo writes {STORAGE_VERSION}",
            STORAGE_VERSION + 1
        )
    );
}

#[test]
fn a_file_from_before_the_versions_does_not_open() {
    let error = open_with_version(None);
    assert_eq!(
        error.downcast_ref::<OtherStorageVersion>(),
        Some(&OtherStorageVersion { found_version: 0 })
    );
}
