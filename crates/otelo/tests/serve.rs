#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use common::{
    Daemon, StopSignal, send_get_request, start_daemon, start_daemon_with_args, stop_daemon,
};
use otelo_indexed_storage::RangeQueries;
use otelo_indexed_storage::query::{PageRequest, SpanSort};
use otelo_indexed_storage_sqlite::{STORAGE_VERSION, TELEMETRY_FILE_NAME};

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
    let now = otelo_indexed_storage::now_unix_nanos();
    let day = 86_400 * 1_000_000_000;
    let reader = otelo_indexed_storage_sqlite::Reader::open(
        &dir.path().join("telemetry"),
        otelo_indexed_storage::TimeRange::new(now - day, now + day).unwrap(),
    )
    .unwrap();
    let list_spans = |query: &str| {
        reader
            .list_spans(
                &otelo_query::parse_query(query, otelo_query::Signal::Spans).unwrap(),
                SpanSort::Oldest,
                &PageRequest::first(1000),
            )
            .unwrap()
            .spans
    };
    let requests = list_spans(r#"service = otelo and name = "GET /api/logs" and root = true"#);
    let [request] = requests.as_slice() else {
        panic!("one request, not {requests:?}");
    };
    let attributes = serde_json::to_value(&request.attributes).unwrap();
    assert_eq!(attributes["http.response.status_code"], 200, "{attributes}");
    assert_eq!(attributes["url.query"], "q=level%20%3E%3D%20warn");
    let children: Vec<String> = reader
        .get_trace(request.trace_id, 1000)
        .unwrap()
        .unwrap()
        .spans
        .into_iter()
        .filter(|span| span.parent_span_id == Some(request.span_id))
        .map(|span| span.name)
        .collect();
    assert_eq!(
        children,
        ["wait for a blocking thread", "open reader", "SELECT"]
    );
    // The rollups and the retention of the writer trace none of their statements.
    assert_eq!(
        list_spans(r#"root = true and name != "GET /api/logs""#).len(),
        0
    );
    let mut bodies: Vec<String> = reader
        .list_logs(
            &otelo_query::parse_query("service = otelo", otelo_query::Signal::Logs).unwrap(),
            &PageRequest::first(1000),
        )
        .unwrap()
        .logs
        .into_iter()
        .map(|log| log.body)
        .collect();
    bodies.reverse();
    // The indexer catches up on its own thread, before or after the daemon listens.
    let (catch_ups, lifecycle): (Vec<&String>, Vec<&String>) = bodies
        .iter()
        .partition(|body| *body == "caught up with the journal");
    assert_eq!(lifecycle, ["listening", "stopping"]);
    assert_eq!(catch_ups.len(), 1, "{bodies:?}");
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

fn send_log(daemon: &Daemon, body: &str) {
    let logged_at = otelo_indexed_storage::now_unix_nanos().to_string();
    let request = format!(
        r#"{{"resourceLogs": [{{"scopeLogs": [{{"logRecords": [
            {{"timeUnixNano": "{logged_at}", "body": {{"stringValue": "{body}"}}}}
        ]}}]}}]}}"#
    );
    let response = ureq::post(format!("http://{}/v1/logs", daemon.otlp_http_addr))
        .header("Content-Type", "application/json")
        .send(request)
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[test]
fn the_daemon_builds_the_telemetry_of_another_storage_version_again_as_it_receives() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    send_log(&daemon, "cart is empty");
    stop_daemon(daemon, StopSignal::Term);
    let telemetry_path = dir.path().join("telemetry").join(TELEMETRY_FILE_NAME);
    rusqlite::Connection::open(&telemetry_path)
        .unwrap()
        .pragma_update(None, "user_version", STORAGE_VERSION + 1)
        .unwrap();

    let daemon = start_daemon(dir.path());
    let warning = format!(
        "telemetry.sqlite has storage version {} and this otelo writes {STORAGE_VERSION}, \
         so the indexer builds it again from the journal",
        STORAGE_VERSION + 1
    );
    assert!(
        daemon.stderr_until_listening.contains(&warning),
        "{}",
        daemon.stderr_until_listening
    );
    send_log(&daemon, "cart is full");
    let daemon_stderr =
        daemon.stderr_until_listening.clone() + &stop_daemon(daemon, StopSignal::Term);
    assert!(
        daemon_stderr.contains("caught up with the journal"),
        "{daemon_stderr}"
    );
    let connection = rusqlite::Connection::open(&telemetry_path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, STORAGE_VERSION);
    let bodies: Vec<String> = connection
        .prepare("SELECT body FROM logs ORDER BY logged_at")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(bodies, ["cart is empty", "cart is full"]);
}

fn open_event_stream(daemon: &Daemon) -> BufReader<TcpStream> {
    let mut stream = TcpStream::connect(&daemon.api_addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request = format!(
        "GET /api/events HTTP/1.1\r\nHost: {}\r\nAuthorization: {}\r\n\r\n",
        daemon.api_addr,
        daemon.bearer_header()
    );
    stream.write_all(request.as_bytes()).unwrap();
    BufReader::new(stream)
}

fn read_next_event_data(events: &mut BufReader<TcpStream>) -> serde_json::Value {
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(events.read_line(&mut line).unwrap(), 0, "the stream ended");
        if let Some(data) = line.trim_end().strip_prefix("data: ") {
            return serde_json::from_str(data).unwrap();
        }
    }
}

#[test]
fn streams_how_far_the_index_has_read_until_the_daemon_stops() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let mut events = open_event_stream(&daemon);
    let caught_up = serde_json::json!({
        "logs": { "state": "caught_up" },
        "spans": { "state": "caught_up" },
        "metrics": { "state": "caught_up" },
    });
    while read_next_event_data(&mut events) != caught_up {}

    stop_daemon(daemon, StopSignal::Term);
    let mut rest = Vec::new();
    events.read_to_end(&mut rest).unwrap();
}
