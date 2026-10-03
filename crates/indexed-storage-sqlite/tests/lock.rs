use std::collections::BTreeSet;

use otelo_indexed_storage_sqlite::Sqlite;

#[test]
fn a_second_storage_on_the_same_files_does_not_open() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(directory.path(), BTreeSet::new()).unwrap();
    let error = Sqlite::open(directory.path(), BTreeSet::new())
        .err()
        .unwrap();
    assert!(
        error.to_string().starts_with("another otelo holds "),
        "{error:#}"
    );
    drop(storage);
    Sqlite::open(directory.path(), BTreeSet::new()).unwrap();
}
