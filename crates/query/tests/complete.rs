use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;

use otelo_query::{
    BuiltinField, Catalog, Expression, Field, FieldHelp, FieldOrigin, FieldValues, KeyInfo, Signal,
    Value, ValueInfo, ValueType, complete_query, parse_query,
};

#[derive(Default)]
struct SmallAppCatalog {
    asked_contexts: RefCell<BTreeSet<String>>,
}

impl SmallAppCatalog {
    fn note_context(&self, context: Option<&Expression>) -> bool {
        let Some(context) = context else {
            return false;
        };
        let text = context.to_string();
        let is_caddy = text.contains("service = \"caddy\"");
        self.asked_contexts.borrow_mut().insert(text);
        is_caddy
    }
}

fn key_info(key: &str, value_type: ValueType, count: u64) -> KeyInfo {
    KeyInfo {
        key: key.into(),
        value_type,
        count,
    }
}

const fn value_info(value: Value, count: u64) -> ValueInfo {
    ValueInfo { value, count }
}

impl Catalog for SmallAppCatalog {
    fn has_builtin_field(&self, _: Signal, _: BuiltinField, context: Option<&Expression>) -> bool {
        !self.note_context(context)
    }

    fn keys(&self, signal: Signal, resource: bool, context: Option<&Expression>) -> Vec<KeyInfo> {
        if self.note_context(context) {
            return if resource {
                vec![key_info("service.name", ValueType::String, 1)]
            } else {
                vec![key_info("http.route", ValueType::String, 2)]
            };
        }
        if resource {
            return vec![
                key_info("service.name", ValueType::String, 4),
                key_info("host.name", ValueType::String, 3),
                key_info("odd key", ValueType::String, 1),
            ];
        }
        match signal {
            Signal::Logs | Signal::Spans => vec![
                key_info("http.route", ValueType::String, 90),
                key_info("http.response.status_code", ValueType::Int, 80),
                key_info("user.id", ValueType::Mixed, 40),
                key_info("level", ValueType::String, 2),
                key_info("odd key", ValueType::String, 1),
            ],
            Signal::Metrics => vec![
                key_info("state", ValueType::String, 6),
                key_info("http.route", ValueType::String, 3),
            ],
        }
    }

    fn values(
        &self,
        signal: Signal,
        field: &Field,
        _: &str,
        context: Option<&Expression>,
    ) -> FieldValues {
        let string_value = |text: &str| Value::String(text.into());
        if self.note_context(context) {
            return FieldValues {
                listed: match field {
                    Field::Attribute(key) if key == "http.route" => {
                        vec![value_info(string_value("/login"), 2)]
                    }
                    _ => Vec::new(),
                },
                has_more_values_than_listed: false,
            };
        }
        let listed = match field {
            Field::Attribute(key) if key == "http.route" => vec![
                value_info(string_value("/matches"), 50),
                value_info(string_value("/languages"), 40),
                value_info(string_value("/login"), 1),
            ],
            Field::Attribute(key) if key == "http.response.status_code" => {
                vec![
                    value_info(Value::Int(200), 70),
                    value_info(Value::Int(500), 10),
                ]
            }
            Field::Attribute(key) if key == "user.id" => {
                vec![
                    value_info(Value::Int(7), 30),
                    value_info(string_value("8"), 10),
                ]
            }
            Field::Resource(key) if key == "host.name" => {
                vec![value_info(string_value("droplet"), 3)]
            }
            Field::Builtin(BuiltinField::Service) => {
                vec![
                    value_info(string_value("mudro"), 9),
                    value_info(string_value("caddy"), 2),
                ]
            }
            Field::Builtin(BuiltinField::Name) if signal == Signal::Spans => {
                vec![
                    value_info(string_value("GET /languages"), 20),
                    value_info(string_value("POST /matches"), 5),
                ]
            }
            Field::Builtin(BuiltinField::Name) => {
                vec![value_info(string_value("http.server.request.duration"), 4)]
            }
            _ => Vec::new(),
        };
        FieldValues {
            listed,
            has_more_values_than_listed: matches!(field, Field::Attribute(key) if key == "user.id"),
        }
    }
}

fn describe_field_help(help: &FieldHelp) -> String {
    let origin = match &help.origin {
        FieldOrigin::Builtin { description } => format!("built-in: {description}"),
        FieldOrigin::Attribute { record_count } => format!("attribute of {record_count} records"),
        FieldOrigin::Resource { resource_count } => {
            format!("attribute of {resource_count} resources")
        }
    };
    let values: Vec<String> = help
        .most_common_values
        .iter()
        .map(|value| {
            value.record_count.map_or_else(
                || value.text.clone(),
                |count| format!("{} in {count}", value.text),
            )
        })
        .collect();
    format!(
        "field {}: {}, {origin}\n  {}{} values: {}",
        help.name,
        help.value_type,
        help.distinct_value_count,
        if help.has_more_values_than_listed {
            "+"
        } else {
            ""
        },
        values.join(", ")
    )
}

fn signal_of_case_file(path: &Path) -> Signal {
    let directory = path.parent().and_then(Path::file_name).unwrap();
    directory.to_str().unwrap().parse().unwrap()
}

#[test]
fn suggestions() {
    insta::glob!("complete/**/*.txt", |path| {
        let signal = signal_of_case_file(path);
        let mut out = String::new();
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let cursor = line
                .find('|')
                .unwrap_or_else(|| panic!("{}: {line:?} has no |", path.display()));
            let input = line.replacen('|', "", 1);
            writeln!(out, "> {line}").unwrap();
            let catalog = SmallAppCatalog::default();
            let completion = complete_query(&input, cursor, signal, &catalog, None);
            for context in catalog.asked_contexts.borrow().iter() {
                writeln!(out, "context: {context}").unwrap();
            }
            if completion.suggestions.is_empty() {
                writeln!(out, "(none)").unwrap();
            }
            for suggestion in completion.suggestions {
                let replaced = &input[suggestion.replaced_byte_range.clone()];
                writeln!(
                    out,
                    "{:<30} {:<8} {:<18} replaces {:?} at {}..{}",
                    suggestion.text,
                    suggestion.kind.name(),
                    suggestion.detail.unwrap_or_default(),
                    replaced,
                    suggestion.replaced_byte_range.start,
                    suggestion.replaced_byte_range.end
                )
                .unwrap();
            }
            if let Some(help) = &completion.help_for_field_at_cursor {
                writeln!(out, "{}", describe_field_help(help)).unwrap();
            }
            writeln!(out).unwrap();
        }
        insta::assert_snapshot!(out);
    });
}

#[test]
fn outer_context_joins_the_context_of_the_cursor() {
    let catalog = SmallAppCatalog::default();
    let outer_context = parse_query("name = \"http.server.request.duration\"", Signal::Metrics)
        .unwrap()
        .expression;
    let input = "state = idle and ";
    let _ = complete_query(
        input,
        input.len(),
        Signal::Metrics,
        &catalog,
        outer_context.as_ref(),
    );
    assert_eq!(
        catalog.asked_contexts.into_inner(),
        BTreeSet::from(["name = \"http.server.request.duration\" AND state = \"idle\"".into()])
    );
}
