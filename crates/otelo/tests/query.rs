#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{StopSignal, send_get_request, start_daemon, stop_daemon};
use otelo_storage::{
    Attributes, Log, LogSource, Records, Resource, Severity, Span, SpanId, SpanKind, SpanStatus,
    TraceContext, TraceId, now_unix_nanos, open_batch_channel,
};
use otelo_storage_sqlite::{Config, Writer};
use serde_json::{Value, json};

fn parse_attributes(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND_NS: i64 = 1_000_000_000;
const TRACE_ID_HEX: &str = "abababababababababababababababab";

fn write_telemetry(data: &Path) {
    let written_at = now_unix_nanos() - 60 * SECOND_NS;
    let build_span = |id: u8, parent: Option<u8>, name: &str, status: SpanStatus| Span {
        trace_id: TraceId([0xab; 16]),
        span_id: SpanId([id; 8]),
        parent_span_id: parent.map(|parent| SpanId([parent; 8])),
        name: name.into(),
        kind: SpanKind::Server,
        started_at: written_at + i64::from(id) * SECOND_NS,
        duration_ns: 20_000_000,
        status,
        attributes: Attributes::new(),
        events: Vec::new(),
    };
    let build_log = |offset: i64, severity: Severity, body: &str, user_id: Option<i64>| Log {
        logged_at: written_at + offset * SECOND_NS,
        severity,
        body: body.into(),
        trace_context: match user_id {
            Some(_) => TraceContext::None,
            None => TraceContext::Span {
                trace_id: TraceId([0xab; 16]),
                span_id: SpanId([2; 8]),
            },
        },
        attributes: user_id.map_or_else(Attributes::new, |id| {
            parse_attributes(json!({"user.id": id, "http.route": "/login"}))
        }),
        source: LogSource::Otlp,
    };
    let records = Records {
        resource: Resource {
            service: "api".into(),
            attributes: Attributes::new(),
        },
        logs: vec![
            build_log(1, Severity::INFO, "user 7 signed in", Some(7)),
            build_log(2, Severity::INFO, "user 8 signed in", Some(8)),
            build_log(3, Severity::ERROR, "query failed", None),
        ],
        spans: vec![
            build_span(1, None, "GET /languages", SpanStatus::Unset),
            build_span(2, Some(1), "SELECT languages", SpanStatus::Error),
        ],
        metrics: Vec::new(),
    };
    let (batch_sender, inbox) = open_batch_channel(1);
    assert!(batch_sender.send_batch(vec![records]));
    let writer = Writer::spawn(Config::new(data.join("telemetry")), inbox).unwrap();
    drop(batch_sender);
    writer.join().unwrap();
}

fn run_otelo(addr: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args(args)
        .env("OTELO_URL", format!("http://{addr}"))
        .output()
        .unwrap()
}

fn run_otelo_and_parse_json(addr: &str, args: &[&str]) -> (Value, String) {
    let output = run_otelo(addr, args);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{args:?} failed: {stderr}");
    (serde_json::from_slice(&output.stdout).unwrap(), stderr)
}

fn check_calls_of_a_service_without_calls(addr: &str) {
    let (calls, _) = run_otelo_and_parse_json(addr, &["calls", "api"]);
    assert_eq!(calls["calls"]["count"], 0);
    assert_eq!(calls["targets"], json!([]));
    assert!(calls.get("buckets").is_none(), "{calls}");
    let table = run_otelo(addr, &["calls", "api", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.starts_with("api: 0 calls"), "{table}");
}

fn check_the_spec_lists_every_path(addr: &str) {
    let spec = send_get_request(addr, "/api/openapi.json");
    for path in [
        "/api/logs",
        "/api/logs/groups",
        "/api/spans",
        "/api/traces",
        "/api/traces/{trace_id}",
        "/api/metrics",
        "/api/metrics/{name}",
        "/api/services",
        "/api/services/{name}",
        "/api/services/{name}/operation",
        "/api/services/{name}/calls",
        "/api/services/{name}/call",
        "/api/attributes",
        "/api/complete",
        "/api/indexes",
        "/api/indexes/{signal}/{key}",
    ] {
        assert!(spec.contains(&format!("\"{path}\"")), "{path} is missing");
    }
}

#[test]
fn the_cli_reads_what_the_api_serves() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.api_addr.clone();

    let (groups, _) = run_otelo_and_parse_json(&addr, &["logs"]);
    assert_eq!(groups["groups"][0]["template"], "user <num> signed in");
    assert_eq!(groups["groups"][0]["count"], 2);

    let (raw, stderr) = run_otelo_and_parse_json(&addr, &["logs", "--raw", "--limit", "1"]);
    assert_eq!(raw["logs"][0]["body"], "query failed");
    assert_eq!(raw["truncated"], true);
    assert!(stderr.contains("--limit"), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");

    let (errors, _) = run_otelo_and_parse_json(&addr, &["logs", "--raw", "level >= error"]);
    assert_eq!(errors["logs"].as_array().unwrap().len(), 1);
    let (user_logs, stderr) = run_otelo_and_parse_json(
        &addr,
        &["logs", "--raw", "user.id", "=", "7", "OR", "user.id = 9"],
    );
    assert_eq!(user_logs["logs"][0]["body"], "user 7 signed in");
    assert_eq!(user_logs["unindexed"], json!(["user.id"]));
    assert!(stderr.contains("otelo index add logs user.id"), "{stderr}");

    let (traces, _) = run_otelo_and_parse_json(&addr, &["traces", "error = true"]);
    assert_eq!(
        traces["traces"][0],
        json!({
            "trace_id": TRACE_ID_HEX,
            "started_at": traces["traces"][0]["started_at"],
            "service": "api",
            "name": "GET /languages",
            "kind": 2,
            "duration_ns": 20_000_000,
            "spans": 2,
            "error": true,
            "attributes": {},
            "resource": {},
        })
    );
    let (spans, _) = run_otelo_and_parse_json(&addr, &["spans", "status = error"]);
    assert_eq!(spans["spans"][0]["name"], "SELECT languages");

    let (trace, _) = run_otelo_and_parse_json(&addr, &["trace", TRACE_ID_HEX]);
    assert_eq!(trace["spans"].as_array().unwrap().len(), 2);
    assert_eq!(trace["logs"][0]["body"], "query failed");

    let tree = run_otelo(&addr, &["trace", TRACE_ID_HEX, "--table"]);
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert!(tree.contains("GET /languages"), "{tree}");
    let child = tree
        .lines()
        .find(|line| line.contains("SELECT languages"))
        .unwrap();
    assert!(child.starts_with("└─ "), "{tree}");
    assert!(child.contains("ERROR"), "{tree}");

    let (metrics, _) = run_otelo_and_parse_json(&addr, &["metrics", "--since", "2d"]);
    assert!(metrics["series"].is_array());

    // Both spans have the server kind, so both are requests.
    let (services, _) = run_otelo_and_parse_json(&addr, &["services"]);
    let api = &services["services"][0];
    assert_eq!(api["service"], "api");
    assert_eq!(api["stats"]["requests"]["count"], 2);
    assert_eq!(api["stats"]["requests"]["errors"], 1);
    assert_eq!(api["stats"]["error_logs"], 1);
    assert!(api.get("buckets").is_none(), "{api}");
    let (services, _) =
        run_otelo_and_parse_json(&addr, &["services", "--buckets", "--step", "10m"]);
    assert_eq!(services["step_ns"], 600 * SECOND_NS);
    assert!(
        !services["services"][0]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let (service, _) = run_otelo_and_parse_json(&addr, &["service", "api"]);
    assert_eq!(service["operations"][0]["name"], "GET /languages");
    assert_eq!(service["operations"][1]["requests"]["errors"], 1);
    let table = run_otelo(&addr, &["service", "api", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.contains("GET /languages"), "{table}");
    check_calls_of_a_service_without_calls(&addr);
    check_the_spec_lists_every_path(&addr);
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_cli_completes_queries_and_lists_attributes() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.api_addr.clone();

    let (completions, _) =
        run_otelo_and_parse_json(&addr, &["complete", "logs", "user.id = 7 AND http.r"]);
    assert_eq!(
        completions["suggestions"],
        json!([{"text": "http.route", "start": 16, "end": 22, "kind": "field", "detail": "string, 2"}])
    );
    let (values, _) = run_otelo_and_parse_json(&addr, &["complete", "logs", "user.id = "]);
    let texts: Vec<&str> = values["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|suggestion| suggestion["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["7", "8"]);
    // The cursor counts characters, and a suggestion replaces the word at it.
    let (completions_at_cursor, _) = run_otelo_and_parse_json(
        &addr,
        &["complete", "logs", "ú = 1 OR use", "--cursor", "11"],
    );
    assert_eq!(completions_at_cursor["suggestions"][0]["start"], 9);

    let (at_attribute, _) = run_otelo_and_parse_json(&addr, &["complete", "logs", "http.route"]);
    assert_eq!(
        at_attribute["field"],
        json!({
            "name": "http.route",
            "source": "attribute",
            "count": 2,
            "type": "string",
            "values": [{"text": "\"/login\"", "count": 2}],
            "distinct_values": 1,
            "has_more_values": false,
        })
    );
    let (at_builtin_field, _) = run_otelo_and_parse_json(&addr, &["complete", "spans", "root"]);
    assert_eq!(
        at_builtin_field["field"],
        json!({
            "name": "root",
            "source": "builtin",
            "description": "Whether the span starts its trace, which means it has no parent.",
            "type": "bool",
            "values": [{"text": "true", "count": null}, {"text": "false", "count": null}],
            "distinct_values": 2,
            "has_more_values": false,
        })
    );

    let (attributes, _) = run_otelo_and_parse_json(&addr, &["attributes", "logs"]);
    assert_eq!(attributes["record"][0]["key"], "http.route");
    assert_eq!(attributes["record"][0]["type"], "string");
    assert_eq!(attributes["record"][1]["indexed"], false);
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn an_index_is_stored_and_applied_to_the_day_files() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.api_addr.clone();

    let (list, _) = run_otelo_and_parse_json(&addr, &["index", "add", "logs", "user.id"]);
    assert_eq!(
        list["indexes"],
        json!([{"signal": "logs", "key": "user.id"}])
    );
    let (user_logs, stderr) = run_otelo_and_parse_json(&addr, &["logs", "--raw", "user.id = 7"]);
    assert_eq!(user_logs["unindexed"], json!([]));
    assert!(stderr.is_empty(), "{stderr}");
    let is_user_id_indexed = || {
        let (attributes, _) = run_otelo_and_parse_json(&addr, &["attributes", "logs"]);
        attributes["record"]
            .as_array()
            .unwrap()
            .iter()
            .find(|attribute| attribute["key"] == "user.id")
            .unwrap()["indexed"]
            .clone()
    };
    assert_eq!(is_user_id_indexed(), true);

    // The writer builds the index within a second or so.
    let day_file_path = dir
        .path()
        .join("telemetry")
        .join(otelo_storage_sqlite::Day::today().file_name());
    let count_attribute_indexes = || -> i64 {
        rusqlite::Connection::open(&day_file_path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name GLOB 'logs_attribute_*'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while count_attribute_indexes() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(count_attribute_indexes(), 1);
    stop_daemon(daemon, StopSignal::Term);

    let daemon = start_daemon(dir.path());
    let addr = daemon.api_addr.clone();
    let listed = run_otelo(&addr, &["index", "list", "--table"]);
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("logs    user.id")
    );
    let (list, _) = run_otelo_and_parse_json(&addr, &["index", "list"]);
    assert_eq!(list["indexes"].as_array().unwrap().len(), 1);
    let (list, _) = run_otelo_and_parse_json(&addr, &["index", "remove", "logs", "user.id"]);
    assert_eq!(list["indexes"], json!([]));
    let output = run_otelo(&addr, &["index", "remove", "logs", "user.id"]);
    assert!(!output.status.success());
    let output = run_otelo(&addr, &["index", "add", "metrics", "state"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("only the attributes of logs and spans"),
        "{stderr}"
    );
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn a_bad_request_prints_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let addr = daemon.api_addr.clone();

    let output = run_otelo(&addr, &["logs", "--since", "yesterday"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("\"yesterday\" is neither a duration"),
        "{stderr}"
    );

    let output = run_otelo(&addr, &["logs", "user.id = = 7"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("column 11: expected a value"), "{stderr}");

    let output = run_otelo(&addr, &["logs", "level = loud"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not a severity"), "{stderr}");

    let output = run_otelo(&addr, &["trace", TRACE_ID_HEX]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no spans or logs"), "{stderr}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn an_unreachable_daemon_is_named() {
    let output = run_otelo("127.0.0.1:1", &["traces"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("http://127.0.0.1:1"), "{stderr}");
}
