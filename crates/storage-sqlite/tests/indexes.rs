use siner_storage::{IndexedAttribute, IndexedSignal, Storage};
use siner_storage_sqlite::Sqlite;

#[test]
fn the_indexed_attributes_outlive_the_storage() {
    let dir = tempfile::tempdir().unwrap();
    let user = IndexedAttribute::new(IndexedSignal::Logs, "user.id").unwrap();
    let route = IndexedAttribute::new(IndexedSignal::Spans, "http.route").unwrap();

    let storage = Sqlite::open(dir.path()).unwrap();
    assert!(storage.indexed_attributes().is_empty());
    storage.add_index(&user).unwrap();
    storage.add_index(&route).unwrap();
    storage.add_index(&user).unwrap();
    assert_eq!(storage.indexed_attributes().len(), 2);
    drop(storage);

    let storage = Sqlite::open(dir.path()).unwrap();
    assert_eq!(storage.indexed_attributes().len(), 2);
    assert!(storage.remove_index(&user).unwrap());
    assert!(!storage.remove_index(&user).unwrap());
    assert_eq!(
        storage.indexed_attributes().into_iter().collect::<Vec<_>>(),
        [route]
    );
}
