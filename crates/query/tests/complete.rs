use std::fmt::Write;
use std::path::Path;

use siner_query::{Builtin, Catalog, Field, KeyInfo, Signal, Value, ValueInfo, complete};

struct SmallAppCatalog;

fn key(key: &str, kind: &str, count: u64) -> KeyInfo {
    KeyInfo {
        key: key.into(),
        kind: kind.into(),
        count,
    }
}

const fn value(value: Value, count: u64) -> ValueInfo {
    ValueInfo { value, count }
}

impl Catalog for SmallAppCatalog {
    fn keys(&self, signal: Signal, resource: bool) -> Vec<KeyInfo> {
        if resource {
            return vec![
                key("service.name", "string", 4),
                key("host.name", "string", 3),
                key("odd key", "string", 1),
            ];
        }
        match signal {
            Signal::Logs | Signal::Spans => vec![
                key("http.route", "string", 90),
                key("http.response.status_code", "int", 80),
                key("user.id", "mixed", 40),
                key("level", "string", 2),
                key("odd key", "string", 1),
            ],
            Signal::Metrics => vec![key("state", "string", 6), key("http.route", "string", 3)],
        }
    }

    fn values(&self, signal: Signal, field: &Field) -> Vec<ValueInfo> {
        let text = |text: &str| Value::String(text.into());
        match field {
            Field::Attribute(key) if key == "http.route" => vec![
                value(text("/matches"), 50),
                value(text("/languages"), 40),
                value(text("/login"), 1),
            ],
            Field::Attribute(key) if key == "http.response.status_code" => {
                vec![value(Value::Int(200), 70), value(Value::Int(500), 10)]
            }
            Field::Attribute(key) if key == "user.id" => {
                vec![value(Value::Int(7), 30), value(text("8"), 10)]
            }
            Field::Resource(key) if key == "host.name" => vec![value(text("droplet"), 3)],
            Field::Builtin(Builtin::Service) => {
                vec![value(text("mudro"), 9), value(text("caddy"), 2)]
            }
            Field::Builtin(Builtin::Name) if signal == Signal::Spans => {
                vec![
                    value(text("GET /languages"), 20),
                    value(text("POST /matches"), 5),
                ]
            }
            Field::Builtin(Builtin::Name) => vec![value(text("http.server.request.duration"), 4)],
            _ => Vec::new(),
        }
    }
}

fn signal_of_case_file(path: &Path) -> Signal {
    let dir = path.parent().and_then(Path::file_name).unwrap();
    dir.to_str().unwrap().parse().unwrap()
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
            let suggestions = complete(&input, cursor, signal, &SmallAppCatalog);
            if suggestions.is_empty() {
                writeln!(out, "(none)").unwrap();
            }
            for s in suggestions {
                let replaced = &input[s.replace.clone()];
                writeln!(
                    out,
                    "{:<30} {:<8} {:<12} replaces {:?} at {}..{}",
                    s.text,
                    s.kind.as_str(),
                    s.detail.unwrap_or_default(),
                    replaced,
                    s.replace.start,
                    s.replace.end
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
        insta::assert_snapshot!(out);
    });
}
