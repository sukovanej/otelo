#![cfg(unix)]

mod common;

use common::{get, start, start_with, stop};

#[test]
fn health_answers_200() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    let response = get(&daemon.addr, "/health");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop(daemon, "TERM");
}

#[test]
fn makes_the_missing_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("var/siner");
    let daemon = start(&data);
    assert!(data.is_dir());
    stop(daemon, "TERM");
}

#[test]
fn writes_telemetry_under_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    stop(start(dir.path()), "TERM");
    let files: Vec<_> = std::fs::read_dir(dir.path().join("telemetry"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        files.iter().any(|name| name.ends_with(".sqlite")),
        "{files:?}"
    );
}

#[test]
fn sigterm_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = stop(start(dir.path()), "TERM");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn ctrl_c_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = stop(start(dir.path()), "INT");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn receives_otlp_on_its_own_ports() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    std::net::TcpStream::connect(&daemon.otlp_grpc).unwrap();
    let ts = siner_telemetry::now().to_string();
    let body = format!(
        r#"{{"resourceLogs": [{{"scopeLogs": [{{"logRecords": [
            {{"timeUnixNano": "{ts}", "body": {{"stringValue": "cart is empty"}}}}
        ]}}]}}]}}"#
    );
    let response = ureq::post(format!("http://{}/v1/logs", daemon.otlp_http))
        .header("Content-Type", "application/json")
        .send(body)
        .unwrap();
    assert_eq!(response.status(), 200);
    stop(daemon, "TERM");
    let day = siner_telemetry::Day::today().file_name();
    let conn = rusqlite::Connection::open(dir.path().join("telemetry").join(day)).unwrap();
    let body: String = conn
        .query_row("SELECT body FROM logs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(body, "cart is empty");
}

#[test]
fn traces_itself() {
    let dir = tempfile::tempdir().unwrap();
    // A first run leaves the day file, so the query has one to read.
    stop(start(dir.path()), "TERM");
    let daemon = start_with(dir.path(), &["--own-telemetry", "self"]);
    let response = get(&daemon.addr, "/api/logs?q=level%20%3E%3D%20warn");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop(daemon, "TERM");
    let day = siner_telemetry::Day::today().file_name();
    let conn = rusqlite::Connection::open(dir.path().join("telemetry").join(day)).unwrap();
    let request: (Vec<u8>, Vec<u8>, String) = conn
        .query_row(
            "SELECT trace_id, span_id, spans.attributes FROM spans
             JOIN resources ON resources.id = resource_id
             WHERE service = 'siner' AND name = 'GET /api/logs' AND parent_span_id IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let (trace_id, span_id, attributes) = request;
    let attributes: serde_json::Value = serde_json::from_str(&attributes).unwrap();
    assert_eq!(attributes["http.response.status_code"], 200, "{attributes}");
    assert_eq!(attributes["url.query"], "q=level%20%3E%3D%20warn");
    let children: Vec<String> = conn
        .prepare(
            "SELECT name FROM spans WHERE trace_id = ?1 AND parent_span_id = ?2 ORDER BY start_ts",
        )
        .unwrap()
        .query_map((&trace_id, &span_id), |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(children, ["open reader", "SELECT"]);
    let bodies: Vec<String> = conn
        .prepare(
            "SELECT body FROM logs JOIN resources ON resources.id = resource_id
             WHERE service = 'siner' ORDER BY ts",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(bodies, ["listening", "stopping"]);
}
