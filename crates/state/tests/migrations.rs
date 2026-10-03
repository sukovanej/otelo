use std::path::Path;

use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use otelo_state::StateFile;
use rusqlite::Connection;

const LAST_STEP: u32 = 2;

fn read_user_version(data_directory: &Path) -> u32 {
    Connection::open(data_directory.join("state.sqlite"))
        .unwrap()
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

fn write_file_of_step_one(data_directory: &Path, user_version: u32) {
    let connection = Connection::open(data_directory.join("state.sqlite")).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE telemetry_indexes (
               signal TEXT NOT NULL,
               key TEXT NOT NULL,
               PRIMARY KEY (signal, key)
             ) WITHOUT ROWID;
             INSERT INTO telemetry_indexes (signal, key) VALUES ('logs', 'user.id');",
        )
        .unwrap();
    connection
        .pragma_update(None, "user_version", user_version)
        .unwrap();
}

#[test]
fn a_new_file_gets_every_step() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    assert_eq!(read_user_version(directory.path()), LAST_STEP);
    assert!(!state.has_password().unwrap());
    drop(state);
    StateFile::open(directory.path()).unwrap();
    assert_eq!(read_user_version(directory.path()), LAST_STEP);
}

#[test]
fn a_file_of_step_one_keeps_its_indexes_and_gets_the_later_steps() {
    for user_version in [0, 1] {
        let directory = tempfile::tempdir().unwrap();
        write_file_of_step_one(directory.path(), user_version);
        let state = StateFile::open(directory.path()).unwrap();
        assert_eq!(read_user_version(directory.path()), LAST_STEP);
        assert_eq!(
            state
                .indexed_attributes()
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>(),
            [IndexedAttribute::new(IndexedSignal::Logs, "user.id").unwrap()]
        );
        state.replace_password().unwrap();
        assert!(state.has_password().unwrap());
    }
}

#[test]
fn a_file_of_a_newer_otelo_does_not_open() {
    let directory = tempfile::tempdir().unwrap();
    write_file_of_step_one(directory.path(), LAST_STEP + 1);
    let error = StateFile::open(directory.path()).err().unwrap();
    assert!(format!("{error:#}").contains("a newer otelo"), "{error:#}");
}
