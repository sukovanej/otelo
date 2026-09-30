use otelo_storage::query::{replace_ids_in_path, replace_values_in_query};

#[test]
fn values_and_ids_are_taken_out() {
    for (query, template) in [
        (
            "SELECT a FROM t WHERE id = 7",
            "SELECT a FROM t WHERE id = ?",
        ),
        ("select 1.5, -2, 'x', 'it''s'", "select ?, -?"),
        (
            "INSERT INTO t VALUES (1, 'a'), (2, 'b')",
            "INSERT INTO t VALUES (?)",
        ),
        (
            r#"SELECT * FROM "2026-09-29".spans WHERE a = $1 AND t2.b = ?2"#,
            r#"SELECT * FROM "2026-09-29".spans WHERE a = ? AND t2.b = ?"#,
        ),
        (
            "SELECT x::text FROM t WHERE id IN (:t0, :t1, :t2) AND ts >= :since",
            "SELECT x::text FROM t WHERE id IN (?) AND ts >= ?",
        ),
        ("GET user:42", "GET user:42"),
        ("  SELECT\n\t1  ", "SELECT ?"),
    ] {
        assert_eq!(replace_values_in_query(query), template, "{query}");
    }
    assert_eq!(
        replace_ids_in_path("/users/42/orders/3fa85f64-5717-4562-b3fc-2c963f66afa6/items/abc"),
        "/users/{id}/orders/{id}/items/abc"
    );
    assert_eq!(replace_ids_in_path("/v1/deadbeef99"), "/v1/{id}");
}
