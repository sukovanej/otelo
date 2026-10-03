use std::fs;
use std::path::Path;

use otelo_indexed_storage::{
    Attributes, Batch, BatchInbox, BatchSender, Buckets, ExplicitBuckets, Histogram,
    HistogramPoint, Log, Metric, NumberPoint, Points, Records, Resource, Severity, Span, SpanEvent,
    SpanId, SpanKind, SpanStatus, Storage, StorageSize, Temporality, TimeRange, TraceContext,
    TraceId, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader, Sqlite, TELEMETRY_FILE_NAME, Writer};
use rusqlite::Connection;
use serde_json::{Value, json};

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;

fn api_resource() -> Resource {
    Resource {
        service: "api".into(),
        attributes: attributes_from_json(json!({"service.name": "api", "host.name": "droplet"})),
    }
}

fn log(logged_at: i64, body: &str) -> Log {
    Log {
        logged_at,
        severity_number: Severity::INFO,
        body: body.into(),
        trace_context: TraceContext::Span {
            trace_id: TraceId([1; 16]),
            span_id: SpanId([2; 8]),
        },
        attributes: attributes_from_json(json!({"user": 7})),
    }
}

fn span(started_at: i64, name: &str) -> Span {
    Span {
        trace_id: TraceId([1; 16]),
        span_id: SpanId([2; 8]),
        parent_span_id: None,
        name: name.into(),
        kind: SpanKind::Server,
        started_at,
        duration_ns: 5_000_000,
        status_code: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: vec![SpanEvent {
            occurred_at: started_at,
            name: "retry".into(),
            attributes: Attributes::new(),
        }],
    }
}

fn number_points(points: &[(i64, f64)]) -> Vec<NumberPoint> {
    points
        .iter()
        .map(|&(recorded_at, value)| NumberPoint { recorded_at, value })
        .collect()
}

fn memory_metric(points: &[(i64, f64)]) -> Metric {
    Metric {
        name: "process.memory.usage".into(),
        unit: "By".into(),
        attributes: attributes_from_json(json!({"state": "used"})),
        points: Points::UpDown(number_points(points)),
    }
}

fn metric_batch(metrics: Vec<Metric>) -> Batch {
    vec![Records {
        resource: api_resource(),
        logs: Vec::new(),
        spans: Vec::new(),
        metrics,
    }]
}

fn log_batch(logged_at: i64, body: &str) -> Batch {
    vec![Records {
        resource: api_resource(),
        logs: vec![log(logged_at, body)],
        spans: Vec::new(),
        metrics: Vec::new(),
    }]
}

fn write_batches(directory: &Path, batches: Vec<Batch>) {
    let (sender, inbox) = open_batch_channel(batches.len().max(1));
    for batch in batches {
        assert!(sender.send_batch(batch));
    }
    write_inbox(directory, sender, inbox);
}

fn write_inbox(directory: &Path, sender: BatchSender, inbox: BatchInbox) {
    let writer = Writer::spawn(Config::new(directory.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn query_pairs<First: rusqlite::types::FromSql, Second: rusqlite::types::FromSql>(
    connection: &Connection,
    sql: &str,
) -> Vec<(First, Second)> {
    connection
        .prepare(sql)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn reads_back_each_kind_across_a_day_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let today = Day::today();
    let before_midnight_at = today.start_at() - SECOND;
    let after_midnight_at = today.start_at() + SECOND;
    let records = vec![Records {
        resource: api_resource(),
        logs: vec![
            log(before_midnight_at, "user 7 signed in"),
            log(after_midnight_at, "payment 12 failed"),
        ],
        spans: vec![
            span(before_midnight_at, "GET /languages"),
            span(after_midnight_at, "POST /matches"),
        ],
        metrics: vec![memory_metric(&[
            (before_midnight_at, 100.0),
            (after_midnight_at, 200.0),
        ])],
    }];
    write_batches(
        directory.path(),
        vec![records, log_batch(after_midnight_at, "user 8 signed in")],
    );

    let reader = Reader::open(
        directory.path(),
        TimeRange::new(before_midnight_at, after_midnight_at + 1).unwrap(),
    )
    .unwrap();
    let connection = reader.connection();
    let bodies: Vec<(i64, String)> = query_pairs(
        connection,
        "SELECT logged_at, body FROM logs ORDER BY logged_at, body",
    );
    assert_eq!(
        bodies,
        [
            (before_midnight_at, "user 7 signed in".into()),
            (after_midnight_at, "payment 12 failed".into()),
            (after_midnight_at, "user 8 signed in".into()),
        ]
    );
    let names: Vec<(i64, String)> = query_pairs(
        connection,
        "SELECT started_at, name FROM spans ORDER BY started_at",
    );
    assert_eq!(
        names,
        [
            (before_midnight_at, "GET /languages".into()),
            (after_midnight_at, "POST /matches".into())
        ]
    );
    let values: Vec<(i64, f64)> = query_pairs(
        connection,
        "SELECT metric_point.recorded_at, metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name = 'process.memory.usage'
         ORDER BY metric_point.recorded_at",
    );
    assert_eq!(
        values,
        [(before_midnight_at, 100.0), (after_midnight_at, 200.0)]
    );
    assert_eq!(
        query_integer(
            connection,
            "SELECT count(*) FROM resources WHERE service = 'api'"
        ),
        1
    );
    let found: Vec<(i64, String)> = query_pairs(
        connection,
        "SELECT log.logged_at, log.body
         FROM log_body_search
         JOIN logs log ON log.rowid = log_body_search.rowid
         WHERE log_body_search MATCH 'payment'",
    );
    assert_eq!(found, [(after_midnight_at, "payment 12 failed".into())]);
}

#[test]
fn a_full_channel_drops_the_batch_and_the_writer_reports_it() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let (sender, inbox) = open_batch_channel(1);
    assert!(sender.send_batch(log_batch(today_start_at, "kept")));
    assert!(!sender.send_batch(log_batch(today_start_at, "dropped")));
    assert_eq!(sender.dropped_batches(), 1);
    write_inbox(directory.path(), sender, inbox);

    let reader = open_reader_of_today(directory.path());
    let connection = reader.connection();
    let bodies: Vec<(i64, String)> = query_pairs(connection, "SELECT logged_at, body FROM logs");
    assert_eq!(bodies, [(today_start_at, "kept".into())]);
    let dropped: Vec<(String, f64)> = query_pairs(
        connection,
        "SELECT metric_series.unit, metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         JOIN resources resource ON resource.id = metric_series.resource_id
         WHERE resource.service = 'otelo'
           AND metric_series.name = 'otelo.telemetry.dropped_batches'",
    );
    assert_eq!(dropped, [("{batch}".into(), 1.0)]);
}

#[test]
fn skips_records_past_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    let expired_at = Day::today().add_days(-7).start_at();
    let kept_at = Day::today().add_days(-6).start_at();
    write_batches(
        directory.path(),
        vec![log_batch(expired_at, "too old"), log_batch(kept_at, "kept")],
    );
    let reader = open_reader_of_today(directory.path());
    let bodies: Vec<(i64, String)> =
        query_pairs(reader.connection(), "SELECT logged_at, body FROM logs");
    assert_eq!(bodies, [(kept_at, "kept".into())]);
}

#[test]
fn a_reader_needs_the_telemetry_file() {
    let directory = tempfile::tempdir().unwrap();
    let range = TimeRange::new(0, SECOND).unwrap();
    assert!(Reader::open(&directory.path().join("telemetry"), range).is_err());
    Sqlite::open(directory.path()).unwrap();
    let reader = Reader::open(&directory.path().join("telemetry"), range).unwrap();
    assert_eq!(
        query_integer(reader.connection(), "SELECT count(*) FROM logs"),
        0
    );
}

#[test]
fn a_reader_cannot_write() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    write_batches(directory.path(), vec![log_batch(today_start_at, "kept")]);
    let reader = open_reader_of_today(directory.path());
    let result = reader.connection().execute("DELETE FROM logs", []);
    assert!(result.is_err());
}

fn open_reader_of_today(directory: &Path) -> Reader {
    let today_start_at = Day::today().start_at();
    Reader::open(
        directory,
        TimeRange::new(today_start_at, today_start_at + 86_400 * SECOND).unwrap(),
    )
    .unwrap()
}

fn query_integer(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn stores_the_kind_and_the_temporality_of_each_series() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let metric = |name: &str, points: Points| Metric {
        name: name.into(),
        unit: "1".into(),
        attributes: Attributes::new(),
        points,
    };
    let one_point = || number_points(&[(today_start_at, 1.0)]);
    let duration_point = HistogramPoint {
        recorded_at: today_start_at,
        histogram: Histogram {
            count: 3,
            sum: Some(2.5),
            min: None,
            max: None,
            buckets: Buckets::Explicit(ExplicitBuckets::new(vec![1.0], vec![2, 1]).unwrap()),
        },
    };
    write_batches(
        directory.path(),
        vec![metric_batch(vec![
            metric("queue.lag", Points::Gauge(one_point())),
            metric("memory.used", Points::UpDown(one_point())),
            metric(
                "emails.sent",
                Points::Counter(Temporality::Cumulative, one_point()),
            ),
            metric(
                "bytes.sent",
                Points::Counter(Temporality::Delta, one_point()),
            ),
            metric(
                "request.duration",
                Points::Histogram(Temporality::Delta, vec![duration_point]),
            ),
        ])],
    );
    let reader = open_reader_of_today(directory.path());
    let kinds: Vec<(String, Option<String>)> = query_pairs(
        reader.connection(),
        "SELECT name || ' ' || kind, aggregation_temporality FROM metric_series
         WHERE name NOT LIKE 'otelo.%' ORDER BY name",
    );
    assert_eq!(
        kinds,
        [
            ("bytes.sent counter".into(), Some("delta".into())),
            ("emails.sent counter".into(), Some("cumulative".into())),
            ("memory.used updown".into(), None),
            ("queue.lag gauge".into(), None),
            ("request.duration histogram".into(), Some("delta".into())),
        ]
    );
    let counts_and_sums: Vec<(String, f64)> = query_pairs(
        reader.connection(),
        "SELECT metric_point.histogram ->> '$.counts', metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name = 'request.duration'",
    );
    assert_eq!(counts_and_sums, [("[2,1]".into(), 2.5)]);
}

#[test]
fn a_batch_written_twice_leaves_each_point_once() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let memory_batch = |values: [f64; 2]| {
        metric_batch(vec![memory_metric(&[
            (today_start_at, values[0]),
            (today_start_at + SECOND, values[1]),
        ])])
    };
    write_batches(
        directory.path(),
        vec![memory_batch([100.0, 200.0]), memory_batch([150.0, 200.0])],
    );
    let reader = open_reader_of_today(directory.path());
    let names_and_values: Vec<(String, f64)> = query_pairs(
        reader.connection(),
        "SELECT metric_series.name, metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name = 'process.memory.usage'
         ORDER BY metric_point.recorded_at",
    );
    let name = || String::from("process.memory.usage");
    assert_eq!(names_and_values, [(name(), 150.0), (name(), 200.0)]);
}

#[test]
fn a_metric_past_1000_series_rejects_the_points_of_its_newer_series() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    let cart_adds_of_user = |user: i64, recorded_at: i64| Metric {
        name: "cart.adds".into(),
        unit: "{item}".into(),
        attributes: attributes_from_json(json!({"user.id": user})),
        points: Points::Gauge(number_points(&[(recorded_at, 1.0)])),
    };
    write_batches(
        directory.path(),
        vec![
            metric_batch(
                (0..1001)
                    .map(|user| cart_adds_of_user(user, today_start_at))
                    .collect(),
            ),
            metric_batch(vec![
                cart_adds_of_user(0, today_start_at + SECOND),
                cart_adds_of_user(2000, today_start_at + SECOND),
                memory_metric(&[(today_start_at, 1.0)]),
            ]),
        ],
    );
    let reader = open_reader_of_today(directory.path());
    let connection = reader.connection();
    let join_cart_adds_series =
        "JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name = 'cart.adds'";
    assert_eq!(
        query_integer(
            connection,
            "SELECT count(*) FROM metric_series WHERE name = 'cart.adds'"
        ),
        1000
    );
    // A series from before the cap still takes points.
    assert_eq!(
        query_integer(
            connection,
            &format!("SELECT count(*) FROM metric_points metric_point {join_cart_adds_series}")
        ),
        1001
    );
    assert_eq!(
        query_integer(
            connection,
            "SELECT count(*) FROM metric_series WHERE name = 'process.memory.usage'"
        ),
        1
    );
    let rejected_points_counter: Vec<(String, f64)> = query_pairs(
        connection,
        "SELECT metric_series.kind || ' ' || metric_series.unit, metric_point.value
         FROM metric_points metric_point
         JOIN metric_series ON metric_series.id = metric_point.metric_series_id
         WHERE metric_series.name = 'otelo.telemetry.rejected_points'",
    );
    assert_eq!(rejected_points_counter, [("counter {point}".into(), 2.0)]);
}

#[test]
fn the_size_counts_every_file_of_the_telemetry_and_of_the_state() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(directory.path()).unwrap();
    let file_bytes = |path: &Path| fs::metadata(path).unwrap().len();
    let state_bytes = file_bytes(&directory.path().join("state.sqlite"));
    let telemetry_path = directory.path().join("telemetry").join(TELEMETRY_FILE_NAME);
    let telemetry_bytes = file_bytes(&telemetry_path);
    assert!(state_bytes > 0 && telemetry_bytes > 0);
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes,
            state_bytes,
        }
    );

    let mut wal_path = telemetry_path.into_os_string();
    wal_path.push("-wal");
    fs::write(wal_path, [0; 512]).unwrap();
    fs::write(directory.path().join("state.sqlite-wal"), [0; 7]).unwrap();
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes: telemetry_bytes + 512,
            state_bytes: state_bytes + 7,
        }
    );
}
