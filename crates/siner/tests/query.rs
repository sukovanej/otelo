#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{get, start_daemon, stop_daemon};
use serde_json::{Value, json};
use siner_storage::{
    Attributes, Log, Records, Resource, Severity, Span, SpanId, SpanKind, SpanStatus, TraceId,
    batch_channel, now_unix_nanos,
};
use siner_storage_sqlite::{Config, Writer};

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;
const TRACE: &str = "abababababababababababababababab";

fn write_telemetry(data: &Path) {
    let written_at = now_unix_nanos() - 60 * SECOND;
    let span = |id: u8, parent: Option<u8>, name: &str, status: SpanStatus| Span {
        trace_id: TraceId([0xab; 16]),
        span_id: SpanId([id; 8]),
        parent_span_id: parent.map(|p| SpanId([p; 8])),
        name: name.into(),
        kind: SpanKind::Server,
        started_at: written_at + i64::from(id) * SECOND,
        duration_ns: 20_000_000,
        status,
        attributes: Attributes::new(),
        events: Vec::new(),
    };
    let log = |offset: i64, severity: Severity, body: &str, user: Option<i64>| Log {
        logged_at: written_at + offset * SECOND,
        severity,
        body: body.into(),
        trace_id: user.is_none().then_some(TraceId([0xab; 16])),
        span_id: user.is_none().then_some(SpanId([2; 8])),
        attributes: user.map_or_else(Attributes::new, |id| {
            attributes_from_json(json!({"user.id": id, "http.route": "/login"}))
        }),
        source: "otlp",
    };
    let records = Records {
        resource: Resource {
            service: "api".into(),
            attributes: Attributes::new(),
        },
        logs: vec![
            log(1, Severity::INFO, "user 7 signed in", Some(7)),
            log(2, Severity::INFO, "user 8 signed in", Some(8)),
            log(3, Severity::ERROR, "query failed", None),
        ],
        spans: vec![
            span(1, None, "GET /languages", SpanStatus::Unset),
            span(2, Some(1), "SELECT languages", SpanStatus::Error),
        ],
        metrics: Vec::new(),
    };
    let (sender, inbox) = batch_channel(1);
    assert!(sender.send(vec![records]));
    let writer = Writer::spawn(Config::new(data.join("telemetry")), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn siner(addr: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_siner"))
        .args(args)
        .env("SINER_URL", format!("http://{addr}"))
        .output()
        .unwrap()
}

fn run_and_parse_json(addr: &str, args: &[&str]) -> (Value, String) {
    let output = siner(addr, args);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{args:?} failed: {stderr}");
    (serde_json::from_slice(&output.stdout).unwrap(), stderr)
}

fn check_calls_of_a_service_without_calls(addr: &str) {
    let (calls, _) = run_and_parse_json(addr, &["calls", "api"]);
    assert_eq!(calls["calls"]["count"], 0);
    assert_eq!(calls["targets"], json!([]));
    assert!(calls.get("buckets").is_none(), "{calls}");
    let table = siner(addr, &["calls", "api", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.starts_with("api: 0 calls"), "{table}");
}

#[test]
fn the_cli_reads_what_the_api_serves() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.addr.clone();

    let (groups, _) = run_and_parse_json(&addr, &["logs"]);
    assert_eq!(groups["groups"][0]["template"], "user <num> signed in");
    assert_eq!(groups["groups"][0]["count"], 2);

    let (raw, stderr) = run_and_parse_json(&addr, &["logs", "--raw", "--limit", "1"]);
    assert_eq!(raw["logs"][0]["body"], "query failed");
    assert_eq!(raw["truncated"], true);
    assert!(stderr.contains("--limit"), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");

    let (errors, _) = run_and_parse_json(&addr, &["logs", "--raw", "level >= error"]);
    assert_eq!(errors["logs"].as_array().unwrap().len(), 1);
    let (user, stderr) = run_and_parse_json(
        &addr,
        &["logs", "--raw", "user.id", "=", "7", "OR", "user.id = 9"],
    );
    assert_eq!(user["logs"][0]["body"], "user 7 signed in");
    assert_eq!(user["unindexed"], json!(["user.id"]));
    assert!(stderr.contains("siner index add logs user.id"), "{stderr}");

    let (traces, _) = run_and_parse_json(&addr, &["traces", "error = true"]);
    assert_eq!(
        traces["traces"][0],
        json!({
            "trace_id": TRACE,
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
    let (spans, _) = run_and_parse_json(&addr, &["spans", "status = error"]);
    assert_eq!(spans["spans"][0]["name"], "SELECT languages");

    let (trace, _) = run_and_parse_json(&addr, &["trace", TRACE]);
    assert_eq!(trace["spans"].as_array().unwrap().len(), 2);
    assert_eq!(trace["logs"][0]["body"], "query failed");

    let tree = siner(&addr, &["trace", TRACE, "--table"]);
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert!(tree.contains("GET /languages"), "{tree}");
    let child = tree
        .lines()
        .find(|l| l.contains("SELECT languages"))
        .unwrap();
    assert!(child.starts_with("└─ "), "{tree}");
    assert!(child.contains("ERROR"), "{tree}");

    let (sql, _) = run_and_parse_json(&addr, &["sql", "SELECT count(*) AS n FROM spans"]);
    assert_eq!(sql["rows"], json!([[2]]));

    let (metrics, _) = run_and_parse_json(&addr, &["metrics", "--since", "2d"]);
    assert!(metrics["series"].is_array());

    // Both spans have the server kind, so both are requests.
    let (services, _) = run_and_parse_json(&addr, &["services"]);
    let api = &services["services"][0];
    assert_eq!(api["service"], "api");
    assert_eq!(api["stats"]["requests"]["count"], 2);
    assert_eq!(api["stats"]["requests"]["errors"], 1);
    assert_eq!(api["stats"]["error_logs"], 1);
    assert!(api.get("buckets").is_none(), "{api}");
    let (services, _) = run_and_parse_json(&addr, &["services", "--buckets", "--step", "10m"]);
    assert_eq!(services["step_ns"], 600 * SECOND);
    assert!(
        !services["services"][0]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let (service, _) = run_and_parse_json(&addr, &["service", "api"]);
    assert_eq!(service["operations"][0]["name"], "GET /languages");
    assert_eq!(service["operations"][1]["requests"]["errors"], 1);
    let table = siner(&addr, &["service", "api", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.contains("GET /languages"), "{table}");
    check_calls_of_a_service_without_calls(&addr);

    let spec = get(&addr, "/api/openapi.json");
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
        "/api/sql",
        "/api/attributes",
        "/api/complete",
        "/api/indexes",
        "/api/indexes/{signal}/{key}",
    ] {
        assert!(spec.contains(&format!("\"{path}\"")), "{path} is missing");
    }
    stop_daemon(daemon, "TERM");
}

#[test]
fn the_cli_completes_queries_and_lists_attributes() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.addr.clone();

    let (completions, _) =
        run_and_parse_json(&addr, &["complete", "logs", "user.id = 7 AND http.r"]);
    assert_eq!(
        completions["suggestions"],
        json!([{"text": "http.route", "start": 16, "end": 22, "kind": "field", "detail": "string, 2"}])
    );
    let (values, _) = run_and_parse_json(&addr, &["complete", "logs", "user.id = "]);
    let texts: Vec<&str> = values["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["7", "8"]);
    // The cursor counts characters, and a suggestion replaces the word at it.
    let (at, _) = run_and_parse_json(
        &addr,
        &["complete", "logs", "ú = 1 OR use", "--cursor", "11"],
    );
    assert_eq!(at["suggestions"][0]["start"], 9);

    let (attributes, _) = run_and_parse_json(&addr, &["attributes", "logs"]);
    assert_eq!(attributes["record"][0]["key"], "http.route");
    assert_eq!(attributes["record"][1]["indexed"], false);
    stop_daemon(daemon, "TERM");
}

#[test]
fn an_index_is_stored_and_applied_to_the_day_files() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());
    let addr = daemon.addr.clone();

    let (list, _) = run_and_parse_json(&addr, &["index", "add", "logs", "user.id"]);
    assert_eq!(
        list["indexes"],
        json!([{"signal": "logs", "key": "user.id"}])
    );
    let (user, stderr) = run_and_parse_json(&addr, &["logs", "--raw", "user.id = 7"]);
    assert_eq!(user["unindexed"], json!([]));
    assert!(stderr.is_empty(), "{stderr}");
    let indexed = || {
        let (attributes, _) = run_and_parse_json(&addr, &["attributes", "logs"]);
        attributes["record"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["key"] == "user.id")
            .unwrap()["indexed"]
            .clone()
    };
    assert_eq!(indexed(), true);

    // The writer builds the index within a second or so.
    let today = siner_storage_sqlite::Day::today();
    let sql = format!("SELECT name FROM \"{today}\".sqlite_master WHERE name GLOB 'attr_*'");
    let index_names = || {
        let (rows, _) = run_and_parse_json(&addr, &["sql", &sql]);
        rows["rows"].as_array().unwrap().len()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while index_names() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(index_names(), 1);
    stop_daemon(daemon, "TERM");

    let daemon = start_daemon(dir.path());
    let addr = daemon.addr.clone();
    let listed = siner(&addr, &["index", "list", "--table"]);
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("logs    user.id")
    );
    let (list, _) = run_and_parse_json(&addr, &["index", "list"]);
    assert_eq!(list["indexes"].as_array().unwrap().len(), 1);
    let (list, _) = run_and_parse_json(&addr, &["index", "remove", "logs", "user.id"]);
    assert_eq!(list["indexes"], json!([]));
    let output = siner(&addr, &["index", "remove", "logs", "user.id"]);
    assert!(!output.status.success());
    let output = siner(&addr, &["index", "add", "metrics", "state"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("only the attributes of logs and spans"),
        "{stderr}"
    );
    stop_daemon(daemon, "TERM");
}

#[test]
fn a_bad_request_prints_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let addr = daemon.addr.clone();

    let output = siner(&addr, &["logs", "--since", "yesterday"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("\"yesterday\" is neither a duration"),
        "{stderr}"
    );

    let output = siner(&addr, &["logs", "user.id = = 7"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("column 11: expected a value"), "{stderr}");

    let output = siner(&addr, &["logs", "level = loud"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not a severity"), "{stderr}");

    let output = siner(&addr, &["sql", "DELETE FROM logs"]);
    assert!(!output.status.success());

    let output = siner(&addr, &["trace", TRACE]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no spans or logs"), "{stderr}");
    stop_daemon(daemon, "TERM");
}

#[test]
fn an_unreachable_daemon_is_named() {
    let output = siner("127.0.0.1:1", &["traces"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("http://127.0.0.1:1"), "{stderr}");
}
