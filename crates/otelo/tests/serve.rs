#![cfg(unix)]

mod common;

use common::{get, start_daemon, start_daemon_with, stop_daemon};

#[test]
fn health_answers_200() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let response = get(&daemon.addr, "/health");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop_daemon(daemon, "TERM");
}

#[test]
fn makes_the_missing_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("var/otelo");
    let daemon = start_daemon(&data);
    assert!(data.is_dir());
    stop_daemon(daemon, "TERM");
}

#[test]
fn writes_telemetry_under_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    stop_daemon(start_daemon(dir.path()), "TERM");
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
    let log = stop_daemon(start_daemon(dir.path()), "TERM");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn ctrl_c_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = stop_daemon(start_daemon(dir.path()), "INT");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn receives_otlp_on_its_own_ports() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    std::net::TcpStream::connect(&daemon.otlp_grpc).unwrap();
    let ts = otelo_storage::now_unix_nanos().to_string();
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
    stop_daemon(daemon, "TERM");
    let day = otelo_storage_sqlite::Day::today().file_name();
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
    stop_daemon(start_daemon(dir.path()), "TERM");
    let daemon = start_daemon_with(dir.path(), &["--own-telemetry", "self"]);
    let response = get(&daemon.addr, "/api/logs?q=level%20%3E%3D%20warn");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop_daemon(daemon, "TERM");
    let day = otelo_storage_sqlite::Day::today().file_name();
    let conn = rusqlite::Connection::open(dir.path().join("telemetry").join(day)).unwrap();
    let request: (Vec<u8>, Vec<u8>, String) = conn
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
    // The rollups of the metrics read the day files too, and trace none of it.
    let outside_a_request: i64 = conn
        .query_row(
            "SELECT count(*) FROM spans WHERE parent_span_id IS NULL AND name != 'GET /api/logs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(outside_a_request, 0);
    let bodies: Vec<String> = conn
        .prepare(
            "SELECT body FROM logs JOIN resources ON resources.id = resource_id
             WHERE service = 'otelo' ORDER BY ts",
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
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon_with(dir.path(), &["--own-telemetry", "self"]);
    let day = otelo_storage_sqlite::Day::today().file_name();
    let path = dir.path().join("telemetry").join(day);
    let host_metrics = "SELECT s.name, s.kind, r.attributes FROM points p
         JOIN series s ON s.id = p.series_id
         JOIN resources r ON r.id = s.resource_id
         WHERE r.service = 'otelo' AND s.name IN
             ('system.memory.limit', 'process.cpu.time', 'otelo.storage.size')
         GROUP BY s.name ORDER BY s.name";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let rows: Vec<(String, String, String)> = loop {
        let rows = rusqlite::Connection::open(&path)
            .and_then(|conn| {
                conn.prepare(host_metrics)?
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
    stop_daemon(daemon, "TERM");

    let kinds: Vec<(&str, &str)> = rows
        .iter()
        .map(|(name, kind, _)| (name.as_str(), kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("otelo.storage.size", "updown"),
            ("process.cpu.time", "counter"),
            ("system.memory.limit", "updown"),
        ]
    );
    let conn = rusqlite::Connection::open(&path).unwrap();
    let resources: Vec<String> = conn
        .prepare("SELECT attributes FROM resources WHERE service = 'otelo'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The collector, and the daemon's own spans and logs.
    assert!(resources.len() >= 2, "{resources:?}");
    for attributes in resources.iter().filter(|json| json.contains("host.")) {
        let attributes: serde_json::Value = serde_json::from_str(attributes).unwrap();
        assert!(attributes["os.type"].is_string(), "{attributes}");
        assert!(attributes["host.arch"].is_string(), "{attributes}");
    }
    let with_host = resources.iter().filter(|json| json.contains("os.type"));
    assert!(with_host.count() >= 2, "{resources:?}");
}
