#![cfg(unix)]

mod common;

use common::{get, start, stop};

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
