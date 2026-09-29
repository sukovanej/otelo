use siner_storage::query::message_template;

#[test]
fn replaces_numbers_and_keeps_their_units() {
    assert_eq!(
        message_template("user 7 paid 12.50 in 340ms"),
        "user <num> paid <num> in <num>ms"
    );
    assert_eq!(
        message_template("GET /users/123 -> 404"),
        "GET /users/<num> -> <num>"
    );
}

#[test]
fn keeps_digits_inside_words() {
    assert_eq!(message_template("http2 user7 v2_api"), "http2 user7 v2_api");
}

#[test]
fn replaces_ids() {
    assert_eq!(
        message_template("match 3f2b8c1e-9d4a-4c1b-8e2f-0a1b2c3d4e5f ended"),
        "match <uuid> ended"
    );
    assert_eq!(
        message_template("trace 4bf92f3577b34da6a3ce929d0e0e4736 at 0x7ff3"),
        "trace <hex> at <hex>"
    );
    assert_eq!(message_template("the deadbeef word"), "the deadbeef word");
}

#[test]
fn replaces_quoted_strings() {
    assert_eq!(
        message_template(r#"unknown language "tlh" and 'kl\'ingon'"#),
        r#"unknown language "<str>" and '<str>'"#
    );
    assert_eq!(message_template("don't stop"), "don't stop");
    assert_eq!(message_template("an \"open quote"), "an \"open quote");
}

#[test]
fn keeps_text_that_is_not_ascii() {
    assert_eq!(message_template("žluťoučký kůň 3×"), "žluťoučký kůň <num>×");
}
