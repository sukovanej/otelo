use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use otelo_state::StateFile;

#[test]
fn the_indexed_attributes_outlive_the_state_file() {
    let directory = tempfile::tempdir().unwrap();
    let user = IndexedAttribute::new(IndexedSignal::Logs, "user.id").unwrap();
    let route = IndexedAttribute::new(IndexedSignal::Spans, "http.route").unwrap();

    let state = StateFile::open(directory.path()).unwrap();
    assert!(state.indexed_attributes().unwrap().is_empty());
    state.add_indexed_attribute(&user).unwrap();
    state.add_indexed_attribute(&route).unwrap();
    state.add_indexed_attribute(&user).unwrap();
    assert_eq!(state.indexed_attributes().unwrap().len(), 2);
    drop(state);

    let state = StateFile::open(directory.path()).unwrap();
    assert_eq!(state.indexed_attributes().unwrap().len(), 2);
    assert!(state.remove_indexed_attribute(&user).unwrap());
    assert!(!state.remove_indexed_attribute(&user).unwrap());
    assert_eq!(
        state
            .indexed_attributes()
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        [route]
    );
}

#[test]
fn the_size_counts_the_write_ahead_log() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    let state_bytes = std::fs::metadata(directory.path().join("state.sqlite"))
        .unwrap()
        .len();
    assert!(state_bytes > 0);
    assert_eq!(state.size_in_bytes().unwrap(), state_bytes);
    std::fs::write(directory.path().join("state.sqlite-wal"), [0; 7]).unwrap();
    assert_eq!(state.size_in_bytes().unwrap(), state_bytes + 7);
}
