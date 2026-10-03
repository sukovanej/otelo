use otelo_state::{SessionToken, StateFile};

#[test]
fn the_file_keeps_only_the_hash_of_the_password() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    let password = state.replace_password().unwrap();
    assert_eq!(password.len(), 26);
    assert!(state.is_password(&password).unwrap());
    assert!(!state.is_password("wrong").unwrap());
    let hash: String = rusqlite::Connection::open(directory.path().join("state.sqlite"))
        .unwrap()
        .query_row("SELECT hash FROM passwords", [], |row| row.get(0))
        .unwrap();
    assert!(hash.starts_with("$argon2id$"), "{hash}");
    assert!(!hash.contains(&password));
}

#[test]
fn a_session_lasts_until_it_ends_or_the_password_changes() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    state.replace_password().unwrap();
    let first = state.start_session().unwrap();
    let second = state.start_session().unwrap();
    assert!(state.renew_session(&first).unwrap());
    assert!(state.renew_session(&second).unwrap());

    state.end_session(&first).unwrap();
    assert!(!state.renew_session(&first).unwrap());
    assert!(state.renew_session(&second).unwrap());

    state.replace_password().unwrap();
    assert!(!state.renew_session(&second).unwrap());
}

#[test]
fn a_session_unused_for_30_days_ends() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    let token = state.start_session().unwrap();
    let days_ago =
        |days: i64| otelo_indexed_storage::now_unix_nanos() - days * 24 * 3600 * 1_000_000_000;
    let connection = rusqlite::Connection::open(directory.path().join("state.sqlite")).unwrap();
    connection
        .execute("UPDATE sessions SET last_used_at = ?1", [days_ago(29)])
        .unwrap();
    assert!(state.renew_session(&token).unwrap());
    connection
        .execute("UPDATE sessions SET last_used_at = ?1", [days_ago(31)])
        .unwrap();
    assert!(!state.renew_session(&token).unwrap());
}

#[test]
fn a_token_reads_back_from_its_text() {
    let directory = tempfile::tempdir().unwrap();
    let state = StateFile::open(directory.path()).unwrap();
    let token = state.start_session().unwrap();
    let text = token.to_string();
    assert_eq!(text.len(), 43);
    assert!(text.parse::<SessionToken>().unwrap() == token);
    assert!("short".parse::<SessionToken>().is_err());
}
