#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use common::{Daemon, StopSignal, send_get_request, start_daemon, stop_daemon};
use otelo_indexed_storage::{
    Attributes, Log, Metric, NumberPoint, PipelineMeters, Points, Records, Resource, Severity,
    Span, SpanId, SpanKind, SpanStatus, TraceContext, TraceId, now_unix_nanos,
};
use otelo_indexed_storage_sqlite::{Config, FrameMapper, index_journal_until_caught_up};
use otelo_journal::Journal;
use otelo_journal_files::JournalFiles;
use otelo_query::Signal;
use serde_json::{Value, json};

fn parse_attributes(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND_NS: i64 = 1_000_000_000;
const TRACE_ID_HEX: &str = "abababababababababababababababab";

fn write_telemetry(data: &Path) {
    let written_at = now_unix_nanos() - 60 * SECOND_NS;
    let build_span = |id: u8, kind: SpanKind, name: &str, status: SpanStatus, attributes| Span {
        trace_id: TraceId([0xab; 16]),
        span_id: SpanId([id; 8]),
        parent_span_id: (id > 1).then_some(SpanId([1; 8])),
        name: name.into(),
        kind,
        started_at: written_at + i64::from(id) * SECOND_NS,
        duration_ns: 20_000_000,
        status_code: status,
        attributes: parse_attributes(attributes),
        events: Vec::new(),
    };
    let build_log = |offset: i64, severity: Severity, body: &str, user_id: Option<i64>| Log {
        logged_at: written_at + offset * SECOND_NS,
        severity_number: severity,
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
    };
    let build_queue_depth = |queue: &str, depth: f64| Metric {
        name: "queue.depth".into(),
        unit: "{job}".into(),
        attributes: parse_attributes(json!({"queue": queue})),
        points: Points::UpDown(vec![NumberPoint {
            recorded_at: written_at,
            value: depth,
        }]),
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
            build_span(
                1,
                SpanKind::Server,
                "GET /languages",
                SpanStatus::Unset,
                json!({"http.request.method": "GET", "http.route": "/languages"}),
            ),
            build_span(
                2,
                SpanKind::Client,
                "SELECT languages",
                SpanStatus::Error,
                json!({"db.system.name": "sqlite", "db.query.text": "SELECT * FROM languages"}),
            ),
        ],
        metrics: vec![
            build_queue_depth("email", 3.0),
            build_queue_depth("sms", 5.0),
            build_queue_depth("push", 1.0),
        ],
    };
    // The frame lives in a journal of its own, which the daemon does not read again.
    let journal_directory = tempfile::tempdir().unwrap();
    let opened = JournalFiles::open(otelo_journal_files::Config::new(
        journal_directory.path().into(),
    ))
    .unwrap();
    let ticket = opened
        .journal
        .append_frame(Signal::Logs, now_unix_nanos(), b"records")
        .unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(ticket.wait_until_synced())
        .unwrap();
    let map_frame: FrameMapper = Arc::new(move |_, _| Ok(vec![records.clone()]));
    index_journal_until_caught_up(
        Config::new(data.join("telemetry")),
        opened.journal,
        map_frame,
        Arc::new(PipelineMeters::default()),
        |_, _| {},
    )
    .unwrap();
    opened.threads.stop_and_join().unwrap();
}

fn run_otelo(daemon: &Daemon, args: &[&str]) -> Output {
    daemon.otelo_command().args(args).output().unwrap()
}

fn run_otelo_and_parse_json(daemon: &Daemon, args: &[&str]) -> (Value, String) {
    let output = run_otelo(daemon, args);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "{args:?} failed: {stderr}");
    (serde_json::from_slice(&output.stdout).unwrap(), stderr)
}

fn check_the_routes_and_the_queries_of_a_service(daemon: &Daemon) {
    let (service, _) = run_otelo_and_parse_json(daemon, &["service", "api"]);
    assert_eq!(service["stats"]["requests"]["count"], 1);
    assert_eq!(
        service["routes"][0]["values"],
        json!({"http.request.method": "GET", "http.route": "/languages"})
    );
    assert_eq!(service["routes"].as_array().unwrap().len(), 1);
    assert_eq!(
        service["queries"][0]["values"],
        json!({"db.system.name": "sqlite", "db.query.text": "SELECT * FROM languages"})
    );
    assert_eq!(service["queries"][0]["spans"]["errors"], 1);
    assert!(service.get("buckets").is_none(), "{service}");
    let table = run_otelo(daemon, &["service", "api", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.contains("GET /languages"), "{table}");
    assert!(table.contains("SELECT * FROM languages"), "{table}");
}

fn check_the_groups_of_spans(daemon: &Daemon) {
    let (groups, _) = run_otelo_and_parse_json(daemon, &["spans", "--by", "name", "error = true"]);
    assert_eq!(groups["spans"]["count"], 1);
    assert_eq!(
        groups["groups"][0]["values"],
        json!({"name": "SELECT languages"})
    );
    let table = run_otelo(daemon, &["spans", "--by", "service,name", "--table"]);
    let table = String::from_utf8(table.stdout).unwrap();
    assert!(table.starts_with("SERVICE"), "{table}");
    assert!(table.contains("GET /languages"), "{table}");

    let by_kind = run_otelo(daemon, &["spans", "--by", "kind"]);
    assert!(!by_kind.status.success());
    let stderr = String::from_utf8(by_kind.stderr).unwrap();
    assert!(stderr.contains("not kind"), "{stderr}");
}

fn check_the_groups_of_a_metric(daemon: &Daemon) {
    let queue_depth_args = ["metric", "queue.depth", "--by", "queue", "--top", "2"];
    let (queue_depth, _) = run_otelo_and_parse_json(daemon, &queue_depth_args);
    let keys: Vec<&Value> = queue_depth["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| &group["key"])
        .collect();
    assert_eq!(
        keys,
        [
            &json!({"type": "values", "values": {"queue": "sms"}, "series_count": 1}),
            &json!({"type": "values", "values": {"queue": "email"}, "series_count": 1}),
            &json!({"type": "other", "group_count": 1, "series_count": 1}),
        ]
    );
    let queue_depth_table = run_otelo(daemon, &[&queue_depth_args[..], &["--table"]].concat());
    let queue_depth_table = String::from_utf8(queue_depth_table.stdout).unwrap();
    assert!(
        queue_depth_table.contains("queue.depth updown {job} queue=sms (1 series) every"),
        "{queue_depth_table}"
    );
    assert!(
        queue_depth_table.contains("queue.depth updown {job} other (1 group, 1 series) every"),
        "{queue_depth_table}"
    );
}

fn read_json_response(response: &str) -> Value {
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"), "{response}");
    serde_json::from_str(body).unwrap()
}

fn check_the_pages_of_a_list(daemon: &Daemon) {
    let all_lines = read_json_response(&send_get_request(daemon, "/api/logs?since=2d"));
    let first_page = read_json_response(&send_get_request(daemon, "/api/logs?since=2d&limit=1"));
    let next = first_page["next"].as_str().unwrap();
    let second_page = read_json_response(&send_get_request(
        daemon,
        &format!("/api/logs?since=2d&limit=1&after={next}"),
    ));
    assert_eq!(first_page["logs"][0], all_lines["logs"][0]);
    assert_eq!(second_page["logs"][0], all_lines["logs"][1]);

    let past_the_page_limit = send_get_request(daemon, "/api/spans?limit=1001");
    assert!(
        past_the_page_limit.starts_with("HTTP/1.1 400"),
        "{past_the_page_limit}"
    );
    let not_a_cursor = send_get_request(daemon, "/api/spans?after=newest");
    assert!(not_a_cursor.starts_with("HTTP/1.1 400"), "{not_a_cursor}");
    let many_metrics = send_get_request(daemon, "/api/metrics?limit=10000");
    assert!(many_metrics.starts_with("HTTP/1.1 200"), "{many_metrics}");
}

fn check_the_spec_lists_every_path(daemon: &Daemon) {
    let spec = send_get_request(daemon, "/api/openapi.json");
    for path in [
        "/api/logs",
        "/api/logs/groups",
        "/api/spans",
        "/api/spans/groups",
        "/api/traces",
        "/api/traces/{trace_id}",
        "/api/metrics",
        "/api/metrics/{name}",
        "/api/services",
        "/api/services/{name}",
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

    let (groups, _) = run_otelo_and_parse_json(&daemon, &["logs"]);
    assert_eq!(groups["groups"][0]["template"], "user <num> signed in");
    assert_eq!(groups["groups"][0]["count"], 2);

    let (raw, stderr) = run_otelo_and_parse_json(&daemon, &["logs", "--raw", "--limit", "1"]);
    assert_eq!(raw["logs"][0]["body"], "query failed");
    assert!(raw["next"].is_string(), "{raw}");
    assert!(stderr.contains("--limit"), "{stderr}");
    assert_eq!(stderr.lines().count(), 1, "{stderr}");

    let (errors, _) = run_otelo_and_parse_json(&daemon, &["logs", "--raw", "level >= error"]);
    assert_eq!(errors["logs"].as_array().unwrap().len(), 1);
    let (user_logs, stderr) = run_otelo_and_parse_json(
        &daemon,
        &["logs", "--raw", "user.id", "=", "7", "OR", "user.id = 9"],
    );
    assert_eq!(user_logs["logs"][0]["body"], "user 7 signed in");
    assert_eq!(user_logs["unindexed"], json!(["user.id"]));
    assert!(stderr.contains("otelo index add logs user.id"), "{stderr}");

    let (traces, _) = run_otelo_and_parse_json(&daemon, &["traces", "error = true"]);
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
            "attributes": {"http.request.method": "GET", "http.route": "/languages"},
            "resource": {},
        })
    );
    let (spans, _) = run_otelo_and_parse_json(&daemon, &["spans", "status = error"]);
    assert_eq!(spans["spans"][0]["name"], "SELECT languages");

    let (trace, _) = run_otelo_and_parse_json(&daemon, &["trace", TRACE_ID_HEX]);
    assert_eq!(trace["spans"].as_array().unwrap().len(), 2);
    assert_eq!(trace["logs"][0]["body"], "query failed");

    let tree = run_otelo(&daemon, &["trace", TRACE_ID_HEX, "--table"]);
    let tree = String::from_utf8(tree.stdout).unwrap();
    assert!(tree.contains("GET /languages"), "{tree}");
    let child = tree
        .lines()
        .find(|line| line.contains("SELECT languages"))
        .unwrap();
    assert!(child.starts_with("└─ "), "{tree}");
    assert!(child.contains("ERROR"), "{tree}");

    let (metrics, _) = run_otelo_and_parse_json(&daemon, &["metrics", "--since", "2d"]);
    assert!(metrics["series"].is_array());
    let (services, _) = run_otelo_and_parse_json(&daemon, &["services"]);
    let api = &services["services"][0];
    assert_eq!(api["service"], "api");
    assert_eq!(api["stats"]["requests"]["count"], 1);
    assert_eq!(api["stats"]["requests"]["errors"], 0);
    assert_eq!(api["stats"]["error_logs"], 1);
    assert!(api.get("buckets").is_none(), "{api}");
    let (services, _) =
        run_otelo_and_parse_json(&daemon, &["services", "--buckets", "--step", "10m"]);
    assert_eq!(services["step_ns"], 600 * SECOND_NS);
    assert!(
        !services["services"][0]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    check_the_routes_and_the_queries_of_a_service(&daemon);
    check_the_groups_of_spans(&daemon);
    check_the_groups_of_a_metric(&daemon);
    check_the_pages_of_a_list(&daemon);
    check_the_spec_lists_every_path(&daemon);
    stop_daemon(daemon, StopSignal::Term);
}

fn check_completions_in_a_context_and_a_range(daemon: &Daemon) {
    let list_suggestion_texts = |args: &[&str]| -> Vec<String> {
        let (completions, _) = run_otelo_and_parse_json(daemon, args);
        completions["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|suggestion| suggestion["text"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(
        list_suggestion_texts(&[
            "complete",
            "logs",
            "user.id = ",
            "--context",
            "level = error"
        ])
        .is_empty()
    );
    assert_eq!(
        list_suggestion_texts(&[
            "complete",
            "metrics",
            "queue = ",
            "--context",
            "name = \"queue.depth\""
        ]),
        [r#""email""#, r#""push""#, r#""sms""#]
    );
    assert!(
        list_suggestion_texts(&[
            "complete",
            "metrics",
            "queue = ",
            "--context",
            "name = missing"
        ])
        .is_empty()
    );
    assert_eq!(
        list_suggestion_texts(&[
            "complete",
            "logs",
            "level = info and user.id = ",
            "--since",
            "1h"
        ]),
        ["7", "8"]
    );
    assert!(
        list_suggestion_texts(&[
            "complete",
            "logs",
            "level = info and user.id = ",
            "--until",
            "10m"
        ])
        .is_empty()
    );
    let broken_context = run_otelo(
        daemon,
        &["complete", "logs", "user.id = ", "--context", "level ="],
    );
    assert!(!broken_context.status.success());
    assert!(
        String::from_utf8_lossy(&broken_context.stderr).contains("expected a value"),
        "{}",
        String::from_utf8_lossy(&broken_context.stderr)
    );
}

#[test]
fn the_cli_completes_queries_and_lists_attributes() {
    let dir = tempfile::tempdir().unwrap();
    write_telemetry(dir.path());
    let daemon = start_daemon(dir.path());

    let (completions, _) =
        run_otelo_and_parse_json(&daemon, &["complete", "logs", "user.id = 7 AND http.r"]);
    assert_eq!(
        completions["suggestions"],
        json!([{"text": "http.route", "start": 16, "end": 22, "kind": "field", "detail": "string, 1"}])
    );
    let (values, _) = run_otelo_and_parse_json(&daemon, &["complete", "logs", "user.id = "]);
    let texts: Vec<&str> = values["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|suggestion| suggestion["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["7", "8"]);
    check_completions_in_a_context_and_a_range(&daemon);
    // The cursor counts characters, and a suggestion replaces the word at it.
    let (completions_at_cursor, _) = run_otelo_and_parse_json(
        &daemon,
        &["complete", "logs", "ú = 1 OR use", "--cursor", "11"],
    );
    assert_eq!(completions_at_cursor["suggestions"][0]["start"], 9);

    let (at_attribute, _) = run_otelo_and_parse_json(&daemon, &["complete", "logs", "http.route"]);
    assert_eq!(
        at_attribute["field"],
        json!({
            "name": "http.route",
            "source": "attribute",
            "count": 2,
            "type": "string",
            "values": [{"text": "\"/login\"", "count": 2}],
            "distinct_values": 1,
            "has_more_values_than_listed": false,
        })
    );
    let (at_builtin_field, _) = run_otelo_and_parse_json(&daemon, &["complete", "spans", "root"]);
    assert_eq!(
        at_builtin_field["field"],
        json!({
            "name": "root",
            "source": "builtin",
            "description": "Whether the span starts its trace, which means it has no parent.",
            "type": "bool",
            "values": [{"text": "true", "count": null}, {"text": "false", "count": null}],
            "distinct_values": 2,
            "has_more_values_than_listed": false,
        })
    );

    let (attributes, _) = run_otelo_and_parse_json(&daemon, &["attributes", "logs"]);
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

    let (list, _) = run_otelo_and_parse_json(&daemon, &["index", "add", "logs", "user.id"]);
    assert_eq!(
        list["indexes"],
        json!([{"signal": "logs", "key": "user.id"}])
    );
    let (user_logs, stderr) = run_otelo_and_parse_json(&daemon, &["logs", "--raw", "user.id = 7"]);
    assert_eq!(user_logs["unindexed"], json!([]));
    assert!(stderr.is_empty(), "{stderr}");
    let is_user_id_indexed = || {
        let (attributes, _) = run_otelo_and_parse_json(&daemon, &["attributes", "logs"]);
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
    let telemetry_file_path = dir
        .path()
        .join("telemetry")
        .join(otelo_indexed_storage_sqlite::TELEMETRY_FILE_NAME);
    let count_attribute_indexes = || -> i64 {
        rusqlite::Connection::open(&telemetry_file_path)
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
    let listed = run_otelo(&daemon, &["index", "list", "--table"]);
    assert!(
        String::from_utf8(listed.stdout)
            .unwrap()
            .contains("logs    user.id")
    );
    let (list, _) = run_otelo_and_parse_json(&daemon, &["index", "list"]);
    assert_eq!(list["indexes"].as_array().unwrap().len(), 1);
    let (list, _) = run_otelo_and_parse_json(&daemon, &["index", "remove", "logs", "user.id"]);
    assert_eq!(list["indexes"], json!([]));
    let output = run_otelo(&daemon, &["index", "remove", "logs", "user.id"]);
    assert!(!output.status.success());
    let output = run_otelo(&daemon, &["index", "add", "metrics", "state"]);
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

    let output = run_otelo(&daemon, &["logs", "--since", "yesterday"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("\"yesterday\" is neither a duration"),
        "{stderr}"
    );

    let output = run_otelo(&daemon, &["logs", "user.id = = 7"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("column 11: expected a value"), "{stderr}");

    let output = run_otelo(&daemon, &["logs", "level = loud"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not a severity"), "{stderr}");

    let output = run_otelo(&daemon, &["trace", TRACE_ID_HEX]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no spans or logs"), "{stderr}");

    let output = run_otelo(&daemon, &["metric", "queue.depth", "--by", "state,name"]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("share its name"), "{stderr}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn an_unreachable_daemon_is_named() {
    let output = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .arg("traces")
        .env("OTELO_URL", "http://127.0.0.1:1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("http://127.0.0.1:1"), "{stderr}");
}
