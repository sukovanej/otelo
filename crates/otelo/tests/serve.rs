#![cfg(unix)]

mod common;

use common::{StopSignal, send_get_request, start_daemon, start_daemon_with_args, stop_daemon};

#[test]
fn health_answers_200() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let response = send_get_request(&daemon, "/health");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn makes_the_missing_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("var/otelo");
    let daemon = start_daemon(&data);
    assert!(data.is_dir());
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn writes_telemetry_under_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    stop_daemon(start_daemon(dir.path()), StopSignal::Term);
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
    let daemon_stderr = stop_daemon(start_daemon(dir.path()), StopSignal::Term);
    assert!(daemon_stderr.contains("stopped"), "{daemon_stderr}");
}

#[test]
fn ctrl_c_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let daemon_stderr = stop_daemon(start_daemon(dir.path()), StopSignal::Int);
    assert!(daemon_stderr.contains("stopped"), "{daemon_stderr}");
}

#[test]
fn receives_otlp_on_its_own_ports() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    std::net::TcpStream::connect(&daemon.otlp_grpc_addr).unwrap();
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
    let connection = rusqlite::Connection::open(
        dir.path()
            .join("telemetry")
            .join(otelo_indexed_storage_sqlite::TELEMETRY_FILE_NAME),
    )
    .unwrap();
    let body: String = connection
        .query_row("SELECT body FROM logs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(body, "cart is empty");
}

#[test]
fn journals_the_otlp_it_receives_and_the_host_metrics() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let response = ureq::post(format!("http://{}/v1/logs", daemon.otlp_http_addr))
        .header("Content-Type", "application/json")
        .send(r#"{"resourceLogs": []}"#)
        .unwrap();
    assert_eq!(response.status(), 200);
    stop_daemon(daemon, StopSignal::Term);
    for signal_directory in ["logs", "metrics"] {
        let segment_bytes: u64 =
            std::fs::read_dir(dir.path().join("journal").join(signal_directory))
                .unwrap()
                .map(|entry| entry.unwrap().metadata().unwrap().len())
                .sum();
        assert!(segment_bytes > 0, "{signal_directory}");
    }
}

#[test]
fn traces_itself() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon_with_args(dir.path(), &["--own-telemetry", "self"]);
    let response = send_get_request(&daemon, "/api/logs?q=level%20%3E%3D%20warn");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop_daemon(daemon, StopSignal::Term);
    let connection = rusqlite::Connection::open(
        dir.path()
            .join("telemetry")
            .join(otelo_indexed_storage_sqlite::TELEMETRY_FILE_NAME),
    )
    .unwrap();
    let request: (Vec<u8>, Vec<u8>, String) = connection
        .query_row(
            "SELECT trace_id, span_id, spans.attributes FROM spans
             JOIN resources ON resources.id = resource_id
             WHERE service = 'otelo' AND name = 'GET /api/logs' AND parent_span_id IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    let (trace_id, span_id, attributes) = request;
    let attributes: serde_json::Value = serde_json::from_str(&attributes).unwrap();
    assert_eq!(attributes["http.response.status_code"], 200, "{attributes}");
    assert_eq!(attributes["url.query"], "q=level%20%3E%3D%20warn");
    let children: Vec<String> = connection
        .prepare(
            "SELECT name FROM spans WHERE trace_id = ?1 AND parent_span_id = ?2 ORDER BY started_at",
        )
        .unwrap()
        .query_map((&trace_id, &span_id), |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(children, ["open reader", "SELECT"]);
    // The rollups and the retention of the writer trace none of their statements.
    let spans_outside_a_request: i64 = connection
        .query_row(
            "SELECT count(*) FROM spans WHERE parent_span_id IS NULL AND name != 'GET /api/logs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(spans_outside_a_request, 0);
    let bodies: Vec<String> = connection
        .prepare(
            "SELECT body FROM logs JOIN resources ON resources.id = resource_id
             WHERE service = 'otelo' ORDER BY logged_at",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(bodies, ["listening", "stopping"]);
}

#[test]
fn collects_the_metrics_of_its_host_when_it_starts() {
    let directory = tempfile::tempdir().unwrap();
    let daemon = start_daemon_with_args(directory.path(), &["--own-telemetry", "self"]);
    let telemetry_file_path = directory
        .path()
        .join("telemetry")
        .join(otelo_indexed_storage_sqlite::TELEMETRY_FILE_NAME);
    let select_host_metrics = "SELECT metric_series.name, metric_series.kind, resource.attributes
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE resource.service = 'otelo' AND metric_series.name IN
             ('system.memory.limit', 'process.cpu.time', 'otelo.storage.size')
         GROUP BY metric_series.name
         ORDER BY metric_series.name";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let rows: Vec<(String, String, String)> = loop {
        let rows = rusqlite::Connection::open(&telemetry_file_path)
            .and_then(|connection| {
                connection
                    .prepare(select_host_metrics)?
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default();
        if rows.len() == 3 {
            break rows;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no host metrics after 10 s, only {rows:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    stop_daemon(daemon, StopSignal::Term);

    let names_and_kinds: Vec<(&str, &str)> = rows
        .iter()
        .map(|(name, kind, _)| (name.as_str(), kind.as_str()))
        .collect();
    assert_eq!(
        names_and_kinds,
        [
            ("otelo.storage.size", "updown"),
            ("process.cpu.time", "counter"),
            ("system.memory.limit", "updown"),
        ]
    );
    let connection = rusqlite::Connection::open(&telemetry_file_path).unwrap();
    let resource_attributes: Vec<String> = connection
        .prepare("SELECT attributes FROM resources WHERE service = 'otelo'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The collector, and the daemon's own spans and logs.
    assert!(resource_attributes.len() >= 2, "{resource_attributes:?}");
    for json in resource_attributes
        .iter()
        .filter(|json| json.contains("host."))
    {
        let attributes: serde_json::Value = serde_json::from_str(json).unwrap();
        assert!(attributes["os.type"].is_string(), "{attributes}");
        assert!(attributes["host.arch"].is_string(), "{attributes}");
    }
    let with_host = resource_attributes
        .iter()
        .filter(|json| json.contains("os.type"));
    assert!(with_host.count() >= 2, "{resource_attributes:?}");
}
