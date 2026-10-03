use otelo_state::StateFile;

#[test]
fn the_file_keeps_only_the_hash_of_the_password() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    assert!(!state.has_password().unwrap());
    let password = state.replace_password().unwrap();
    assert_eq!(password.len(), 26);
    assert!(state.has_password().unwrap());
    assert!(state.is_password(&password).unwrap());
    assert!(!state.is_password("WRONGPASSWORD").unwrap());
    let hash: Vec<u8> = rusqlite::Connection::open(directory.path().join("state.sqlite"))
        .unwrap()
        .query_row("SELECT hash FROM passwords", [], |row| row.get(0))
        .unwrap();
    assert_eq!(hash.len(), 32);
}

#[test]
fn a_new_password_replaces_the_old_one() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    let old_password = state.replace_password().unwrap();
    let new_password = state.replace_password().unwrap();
    assert_ne!(old_password, new_password);
    assert!(!state.is_password(&old_password).unwrap());
    assert!(state.is_password(&new_password).unwrap());
}
