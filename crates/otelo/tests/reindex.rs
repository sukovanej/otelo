#![cfg(unix)]

mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{StopSignal, start_daemon, stop_daemon};
use otelo_indexed_storage_sqlite::TELEMETRY_FILE_NAME;

fn run_reindex(data_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args(["reindex", "--data"])
        .arg(data_dir)
        .output()
        .unwrap()
}

#[test]
fn rebuilds_the_telemetry_of_another_storage_version_from_the_journal() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let logged_at = otelo_indexed_storage::now_unix_nanos().to_string();
    let body = format!(
        r#"{{"resourceLogs": [{{"scopeLogs": [{{"logRecords": [
            {{"timeUnixNano": "{logged_at}", "body": {{"stringValue": "cart is empty"}}}}
        ]}}]}}]}}"#
    );
    let response = ureq::post(format!("http://{}/v1/logs", daemon.otlp_http_addr))
        .header("Content-Type", "application/json")
        .send(body)
        .unwrap();
    assert_eq!(response.status(), 200);
    stop_daemon(daemon, StopSignal::Term);
    let telemetry_path = dir.path().join("telemetry").join(TELEMETRY_FILE_NAME);
    rusqlite::Connection::open(&telemetry_path)
        .unwrap()
        .pragma_update(None, "user_version", 0)
        .unwrap();

    let output = run_reindex(dir.path());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next(),
        Some("deleted the telemetry of storage version 0")
    );
    assert!(
        lines.clone().any(|line| line.starts_with("logs 20")),
        "{stdout}"
    );
    let connection = rusqlite::Connection::open(&telemetry_path).unwrap();
    let body: String = connection
        .query_row("SELECT body FROM logs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(body, "cart is empty");

    stop_daemon(start_daemon(dir.path()), StopSignal::Term);
}

#[test]
fn refuses_to_run_while_the_daemon_runs() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let output = run_reindex(dir.path());
    stop_daemon(daemon, StopSignal::Term);
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("another otelo holds "), "{stderr}");
}
