use siner_query::Signal;
use siner_state::State;
use siner_telemetry::IndexedKey;

#[test]
fn the_indexed_attributes_outlive_the_state() {
    let dir = tempfile::tempdir().unwrap();
    let user = IndexedKey::new(Signal::Logs, "user.id").unwrap();
    let route = IndexedKey::new(Signal::Spans, "http.route").unwrap();

    let state = State::open(dir.path()).unwrap();
    assert!(state.indexes().unwrap().is_empty());
    state.add_index(&user).unwrap();
    state.add_index(&route).unwrap();
    // Adding a key again keeps one.
    state.add_index(&user).unwrap();
    drop(state);

    let state = State::open(dir.path()).unwrap();
    assert_eq!(state.indexes().unwrap().len(), 2);
    assert!(state.remove_index(&user).unwrap());
    assert!(!state.remove_index(&user).unwrap());
    assert_eq!(
        state.indexes().unwrap().into_iter().collect::<Vec<_>>(),
        [route]
    );
}
