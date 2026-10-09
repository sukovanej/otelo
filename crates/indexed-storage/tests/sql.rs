use otelo_indexed_storage::{is_sql_system, sanitize_sql_query_text};

fn sanitize(text: &str) -> String {
    sanitize_sql_query_text(text).unwrap_or_else(|| text.to_owned())
}

#[test]
fn replaces_the_literals_of_a_query_with_placeholders() {
    assert_eq!(
        sanitize("SELECT * FROM users WHERE name = 'O''Brien' AND age > 30 AND score < -2.5e3"),
        "SELECT * FROM users WHERE name = ? AND age > ? AND score < -?"
    );
    assert_eq!(
        sanitize("SELECT * FROM blobs WHERE hash = X'0aff' OR flags = 0x1f LIMIT 10"),
        "SELECT * FROM blobs WHERE hash = ? OR flags = ? LIMIT ?"
    );
}

#[test]
fn collapses_a_list_of_values_to_one_placeholder() {
    assert_eq!(
        sanitize("SELECT tag_id FROM tag_parents WHERE tag_id IN (1, 10, 12) ORDER BY tag_id"),
        "SELECT tag_id FROM tag_parents WHERE tag_id IN (?) ORDER BY tag_id"
    );
    assert_eq!(
        sanitize("SELECT tag_id FROM tag_parents WHERE tag_id IN (7) ORDER BY tag_id"),
        "SELECT tag_id FROM tag_parents WHERE tag_id IN (?) ORDER BY tag_id"
    );
    assert_eq!(
        sanitize("INSERT INTO votes (user_id, score) VALUES (1, 'up'), (2, 'down'), (3, 'up')"),
        "INSERT INTO votes (user_id, score) VALUES (?)"
    );
}

#[test]
fn keeps_a_query_whose_values_are_bound() {
    for query in [
        "SELECT ducats FROM wallets WHERE user_id = ?1 AND kind = ?2",
        "SELECT * FROM t WHERE id IN (?, ?, ?)",
        "SELECT * FROM t WHERE id = $1 OR name = :name OR team = @team",
        "SELECT a, b, c FROM t2 JOIN t3 ON t2.id = t3.id",
    ] {
        assert_eq!(sanitize_sql_query_text(query), None, "{query}");
    }
    assert_eq!(
        sanitize("SELECT * FROM t WHERE id = ?1 AND kind = 3"),
        "SELECT * FROM t WHERE id = ?1 AND kind = ?"
    );
}

#[test]
fn keeps_identifiers_and_comments() {
    assert_eq!(
        sanitize(r#"SELECT "user 2".id, `t1`.x FROM t1 WHERE x = 5 -- 42 answers"#),
        r#"SELECT "user 2".id, `t1`.x FROM t1 WHERE x = ? -- 42 answers"#
    );
}

#[test]
fn knows_the_sql_systems() {
    assert!(is_sql_system("sqlite"));
    assert!(is_sql_system("postgresql"));
    assert!(!is_sql_system("redis"));
}
