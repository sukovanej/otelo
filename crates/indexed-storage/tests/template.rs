use otelo_indexed_storage::query::replace_values_in_message;

#[test]
fn replaces_numbers_and_keeps_their_units() {
    assert_eq!(
        replace_values_in_message("user 7 paid 12.50 in 340ms"),
        "user <num> paid <num> in <num>ms"
    );
    assert_eq!(
        replace_values_in_message("GET /users/123 -> 404"),
        "GET /users/<num> -> <num>"
    );
}

#[test]
fn keeps_digits_inside_words() {
    assert_eq!(
        replace_values_in_message("http2 user7 v2_api"),
        "http2 user7 v2_api"
    );
}

#[test]
fn replaces_ids() {
    assert_eq!(
        replace_values_in_message("match 3f2b8c1e-9d4a-4c1b-8e2f-0a1b2c3d4e5f ended"),
        "match <uuid> ended"
    );
    assert_eq!(
        replace_values_in_message("trace 4bf92f3577b34da6a3ce929d0e0e4736 at 0x7ff3"),
        "trace <hex> at <hex>"
    );
    assert_eq!(
        replace_values_in_message("the deadbeef word"),
        "the deadbeef word"
    );
}

#[test]
fn replaces_quoted_strings() {
    assert_eq!(
        replace_values_in_message(r#"unknown language "tlh" and 'kl\'ingon'"#),
        r#"unknown language "<str>" and '<str>'"#
    );
    assert_eq!(replace_values_in_message("don't stop"), "don't stop");
    assert_eq!(
        replace_values_in_message("an \"open quote"),
        "an \"open quote"
    );
}

#[test]
fn keeps_text_that_is_not_ascii() {
    assert_eq!(
        replace_values_in_message("žluťoučký kůň 3×"),
        "žluťoučký kůň <num>×"
    );
}
