#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{get, start, stop};
use serde_json::{Map, Value, json};
use siner_telemetry::{Config, Log, Records, Resource, Span, Writer, channel};

const SECOND: i64 = 1_000_000_000;
const TRACE: &str = "abababababababababababababababab";

/// Writes a trace of two spans with a log line, and two more log lines, into
/// the day file of the last minutes.
fn write_telemetry(data: &Path) {
    let now = siner_now() - 60 * SECOND;
    let span = |id: u8, parent: Option<u8>, name: &str, status: i32| Span {
        trace_id: [0xab; 16],
        span_id: [id; 8],
        parent_span_id: parent.map(|p| [p; 8]),
        name: name.into(),
        kind: 2,
        start_ts: now + i64::from(id) * SECOND,
        duration_ns: 20_000_000,
        status,
        attributes: Map::new(),
        events: Vec::new(),
    };
    let log = |offset: i64, severity: i32, body: &str, user: Option<i64>| Log {
        ts: now + offset * SECOND,
        severity,
        body: body.into(),
        trace_id: user.is_none().then_some([0xab; 16]),
        span_id: user.is_none().then_some([2; 8]),
        attributes: user.map_or_else(Map::new, |id| {
            json!({"user.id": id, "http.route": "/login"})
                .as_object()
                .unwrap()
                .clone()
        }),
        source: "otlp",
    };
    let records = Records {
        resource: Resource {
            service: "api".into(),
            attributes: Map::new(),
        },
        logs: vec![
            log(1, 9, "user 7 signed in", Some(7)),
            log(2, 9, "user 8 signed in", Some(8)),
            log(3, 17, "query failed", None),
        ],
        spans: vec![
            span(1, None, "GET /languages", 0),
            span(2, Some(1), "SELECT languages", 2),
        ],
        metrics: Vec::new(),
    };
    let (sender, inbox) = channel(1);
    sender.send(vec![records]);
    let writer = Writer::spawn(Config::new(data.join("telemetry")), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn siner_now() -> i64 {
    i64::try_from(jiff::Timestamp::now().as_nanosecond()).unwrap()
}

fn siner(addr: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_siner"))
        .args(args)
        .env("SINER_URL", format!("http://{addr}"))
        .output()
        .unwrap()
}

/// Runs a command that has to succeed and parses its JSON output.
fn json(addr: &str, args: &[&str]) -> (Value, String) {
    let output = siner(addr, args);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{args:?} failed: {stderr}");
    (serde_json::from_slice(&output.stdout).unwrap(), stderr)
}

#[test]
fn the_cli_reads_what_the_api_serves() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start(dir.path());
    let addr = daemon.addr.clone();

    // Not a terminal, so JSON.
    let (groups, _) = json(&addr, &["logs"]);
    assert_eq!(groups["groups"][0]["template"], "user <num> signed in");
    assert_eq!(groups["groups"][0]["count"], 2);

    let (raw, stderr) = json(&addr, &["logs", "--raw", "--limit", "1"]);
    assert_eq!(raw["logs"][0]["body"], "query failed");
    assert_eq!(raw["truncated"], true);
    assert!(stderr.contains("--limit"), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");

    // A query in one argument or in several words.
    let (errors, _) = json(&addr, &["logs", "--raw", "level >= error"]);
    assert_eq!(errors["logs"].as_array().unwrap().len(), 1);
    let (user, stderr) = json(
        &addr,
        &["logs", "--raw", "user.id", "=", "7", "OR", "user.id = 9"],
    );
    assert_eq!(user["logs"][0]["body"], "user 7 signed in");
    assert_eq!(user["unindexed"], json!(["user.id"]));
    assert!(stderr.contains("siner index add logs user.id"), "{stderr}");

    let (traces, _) = json(&addr, &["traces", "error = true"]);
    assert_eq!(
        traces["traces"][0],
        json!({
            "trace_id": TRACE,
            "time": traces["traces"][0]["time"],
            "service": "api",
            "name": "GET /languages",
            "duration_ns": 20_000_000,
            "spans": 2,
            "error": true,
        })
    );
    let (spans, _) = json(&addr, &["spans", "status = error"]);
    assert_eq!(spans["spans"][0]["name"], "SELECT languages");

    let (trace, _) = json(&addr, &["trace", TRACE]);
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

    let (sql, _) = json(&addr, &["sql", "SELECT count(*) AS n FROM spans"]);
    assert_eq!(sql["rows"], json!([[2]]));

    let (metrics, _) = json(&addr, &["metrics", "--since", "2d"]);
    assert!(metrics["series"].is_array());

    let spec = get(&addr, "/api/openapi.json");
    for path in [
        "/api/logs",
        "/api/logs/groups",
        "/api/spans",
        "/api/traces",
        "/api/traces/{trace_id}",
        "/api/metrics",
        "/api/metrics/{name}",
        "/api/sql",
        "/api/attributes",
        "/api/complete",
        "/api/indexes",
        "/api/indexes/{signal}/{key}",
    ] {
        assert!(spec.contains(&format!("\"{path}\"")), "{path} is missing");
    }
    stop(daemon, "TERM");
}

#[test]
fn the_cli_completes_queries_and_lists_attributes() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start(dir.path());
    let addr = daemon.addr.clone();

    let (completions, _) = json(&addr, &["complete", "logs", "user.id = 7 AND http.r"]);
    assert_eq!(
        completions["suggestions"],
        json!([{"text": "http.route", "start": 16, "end": 22, "kind": "field", "detail": "string, 2"}])
    );
    let (values, _) = json(&addr, &["complete", "logs", "user.id = "]);
    let texts: Vec<&str> = values["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["7", "8"]);
    // The cursor counts characters, and a suggestion replaces the word at it.
    let (at, _) = json(
        &addr,
        &["complete", "logs", "ú = 1 OR use", "--cursor", "11"],
    );
    assert_eq!(at["suggestions"][0]["start"], 9);

    let (attributes, _) = json(&addr, &["attributes", "logs"]);
    assert_eq!(attributes["record"][0]["key"], "http.route");
    assert_eq!(attributes["record"][1]["indexed"], false);
    stop(daemon, "TERM");
}

#[test]
fn an_index_is_stored_and_applied_to_the_day_files() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start(dir.path());
    let addr = daemon.addr.clone();

    let (list, _) = json(&addr, &["index", "add", "logs", "user.id"]);
    assert_eq!(
        list["indexes"],
        json!([{"signal": "logs", "key": "user.id"}])
    );
    let (user, stderr) = json(&addr, &["logs", "--raw", "user.id = 7"]);
    assert_eq!(user["unindexed"], json!([]));
    assert!(stderr.is_empty(), "{stderr}");
    let indexed = || {
        let (attributes, _) = json(&addr, &["attributes", "logs"]);
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
    let today = siner_telemetry::Day::today();
    let sql = format!("SELECT name FROM \"{today}\".sqlite_master WHERE name GLOB 'attr_*'");
    let index_names = || {
        let (rows, _) = json(&addr, &["sql", &sql]);
        rows["rows"].as_array().unwrap().len()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while index_names() == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(index_names(), 1);
    stop(daemon, "TERM");

    // The set outlives the daemon.
    let daemon = start(dir.path());
    let addr = daemon.addr.clone();
    let listed = siner(&addr, &["index", "list", "--table"]);
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("logs    user.id")
    );
    let (list, _) = json(&addr, &["index", "list"]);
    assert_eq!(list["indexes"].as_array().unwrap().len(), 1);
    let (list, _) = json(&addr, &["index", "remove", "logs", "user.id"]);
    assert_eq!(list["indexes"], json!([]));
    let output = siner(&addr, &["index", "remove", "logs", "user.id"]);
    assert!(!output.status.success());
    let output = siner(&addr, &["index", "add", "metrics", "state"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("only the attributes of logs and spans"),
        "{stderr}"
    );
    stop(daemon, "TERM");
}

#[test]
fn a_bad_request_prints_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
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
    stop(daemon, "TERM");
}

#[test]
fn an_unreachable_daemon_is_named() {
    let output = siner("127.0.0.1:1", &["traces"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("http://127.0.0.1:1"), "{stderr}");
}
