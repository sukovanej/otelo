use std::fmt::Write;
use std::path::Path;

use otelo_query::{BuiltinField, Expression, Field, Operator, Query, Signal, Value, parse_query};

fn signal_of_case_file(path: &Path) -> Signal {
    let directory = path.parent().and_then(Path::file_name).unwrap();
    directory.to_str().unwrap().parse().unwrap()
}

fn describe_resolved_field(field: &Field) -> String {
    match field {
        Field::Builtin(builtin_field) => {
            format!("{} (built-in)", BuiltinField::name(*builtin_field))
        }
        Field::Attribute(key) => format!("{key:?} (attribute)"),
        Field::Resource(key) => format!("{key:?} (resource)"),
    }
}

#[test]
fn queries() {
    insta::glob!("parse/**/*.txt", |path| {
        let signal = signal_of_case_file(path);
        let mut out = String::new();
        for line in std::fs::read_to_string(path).unwrap().lines() {
            writeln!(out, "> {line}").unwrap();
            match parse_query(line, signal) {
                Ok(query) => {
                    writeln!(out, "{query}").unwrap();
                    let fields: Vec<String> = query
                        .fields()
                        .into_iter()
                        .map(describe_resolved_field)
                        .collect();
                    writeln!(out, "fields: {}", fields.join(", ")).unwrap();
                }
                Err(error) => writeln!(out, "error: {error}").unwrap(),
            }
            writeln!(out).unwrap();
        }
        insta::assert_snapshot!(out);
    });
}

fn parse_and_print(signal: Signal, input: &str) -> String {
    parse_query(input, signal).unwrap().to_string()
}

fn parse_error_message(input: &str) -> String {
    parse_query(input, Signal::Logs).unwrap_err().to_string()
}

#[test]
fn parses_the_example() {
    let query = parse_query(
        r#"http.route = "/matches" OR (user.id = 7 AND http.response.status_code = 200)"#,
        Signal::Spans,
    )
    .unwrap();
    let attribute = |key: &str| Field::Attribute(key.into());
    assert_eq!(
        query.expression,
        Some(Expression::Or(vec![
            Expression::Compare {
                field: attribute("http.route"),
                operator: Operator::Eq,
                value: Value::String("/matches".into()),
            },
            Expression::And(vec![
                Expression::Compare {
                    field: attribute("user.id"),
                    operator: Operator::Eq,
                    value: Value::Int(7),
                },
                Expression::Compare {
                    field: attribute("http.response.status_code"),
                    operator: Operator::Eq,
                    value: Value::Int(200),
                },
            ]),
        ]))
    );
}

#[test]
fn and_binds_tighter_than_or_and_terms_join_with_and() {
    assert_eq!(
        parse_and_print(Signal::Logs, "a = 1 or b = 2 c = 3"),
        "a = 1 OR b = 2 AND c = 3"
    );
    assert_eq!(
        parse_and_print(Signal::Logs, "(a = 1 or b = 2) and not c = 3"),
        "(a = 1 OR b = 2) AND NOT c = 3"
    );
}

#[test]
fn resolves_builtins_per_signal_and_the_prefixes() {
    let query = parse_query(
        "name = x resource.host.name = y attr.level = z",
        Signal::Logs,
    )
    .unwrap();
    assert_eq!(
        query.fields(),
        [
            &Field::Attribute("name".into()),
            &Field::Resource("host.name".into()),
            &Field::Attribute("level".into()),
        ]
    );
    let query = parse_query("name = x `odd key` = 1", Signal::Spans).unwrap();
    assert_eq!(
        query.fields(),
        [
            &Field::Builtin(BuiltinField::Name),
            &Field::Attribute("odd key".into())
        ]
    );
    assert_eq!(
        parse_and_print(Signal::Logs, "attr.level = 1 `a b` = 2 attr.and = 3"),
        "attr.level = 1 AND `a b` = 2 AND attr.and = 3"
    );
}

#[test]
fn parses_every_operator_and_value() {
    assert_eq!(
        parse_and_print(
            Signal::Spans,
            r#"duration >= 1.5s status != error error = true x < -2.5 body ~ 'a "b"' has(db.system) k in (1, "two", three)"#,
        ),
        r#"duration >= 1500000000ns AND status != "error" AND error = true AND x < -2.5 AND attr.body ~ "a \"b\"" AND has(db.system) AND k in (1, "two", "three")"#
    );
}

#[test]
fn an_empty_query_keeps_everything() {
    assert_eq!(
        parse_query("  ", Signal::Logs).unwrap(),
        Query::all(Signal::Logs)
    );
}

#[test]
fn says_where_and_what_is_wrong() {
    assert_eq!(
        parse_error_message("user.id"),
        "column 8: expected an operator after user.id, found the end of the query"
    );
    assert_eq!(
        parse_error_message("user.id 7"),
        "column 9: expected an operator after user.id: =, !=, <, <=, >, >=, ~, or in, found \"7\""
    );
    assert_eq!(
        parse_error_message("a = 1 or"),
        "column 9: expected a field, found the end of the query"
    );
    assert_eq!(
        parse_error_message("(a = 1"),
        "column 7: expected ), found the end of the query"
    );
    assert_eq!(
        parse_error_message("a = \"open"),
        "column 5: the string has no closing quote"
    );
    assert_eq!(
        parse_error_message("a = 1 )"),
        "column 7: expected AND, OR, or the end of the query, found \")\""
    );
    assert_eq!(parse_error_message("a ! 1"), "column 3: unexpected '!'");
    assert_eq!(
        parse_error_message("body ~ 5"),
        "column 7: ~ takes a string"
    );
    assert_eq!(
        parse_error_message("k in 1"),
        "column 6: expected ( after in, found \"1\""
    );
}
