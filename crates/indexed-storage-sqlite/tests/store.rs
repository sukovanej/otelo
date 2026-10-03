use std::fs;
use std::path::Path;

use otelo_indexed_storage::{
    Attributes, Batch, BatchInbox, BatchSender, Buckets, ExplicitBuckets, Histogram,
    HistogramPoint, Log, Metric, NumberPoint, Points, Records, Resource, Severity, Span, SpanEvent,
    SpanId, SpanKind, SpanStatus, Storage, StorageSize, Temporality, TimeRange, TraceContext,
    TraceId, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{Config, Day, Reader, Sqlite, Writer};
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
        severity: Severity::INFO,
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
        status: SpanStatus::Unset,
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

fn query_pairs<T: rusqlite::types::FromSql>(
    connection: &Connection,
    sql: &str,
) -> Vec<(String, T)> {
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
    let yesterday = today.add_days(-1);
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
    assert_eq!(reader.days(), [yesterday, today]);
    let connection = reader.connection();
    let (yesterday_name, today_name) = (yesterday.to_string(), today.to_string());

    let bodies: Vec<(String, String)> = query_pairs(
        connection,
        "SELECT day, body FROM logs ORDER BY logged_at, body",
    );
    assert_eq!(
        bodies,
        [
            (yesterday_name.clone(), "user 7 signed in".into()),
            (today_name.clone(), "payment 12 failed".into()),
            (today_name.clone(), "user 8 signed in".into()),
        ]
    );
    let names: Vec<(String, String)> = query_pairs(
        connection,
        "SELECT day, name FROM spans ORDER BY started_at",
    );
    assert_eq!(
        names,
        [
            (yesterday_name.clone(), "GET /languages".into()),
            (today_name.clone(), "POST /matches".into())
        ]
    );
    let values: Vec<(String, f64)> = query_pairs(
        connection,
        "SELECT point.day, point.value
         FROM points point
         JOIN series ON series.day = point.day AND series.id = point.series_id
         WHERE series.name = 'process.memory.usage'
         ORDER BY point.recorded_at",
    );
    assert_eq!(
        values,
        [(yesterday_name.clone(), 100.0), (today_name.clone(), 200.0)]
    );

    let resources: Vec<(String, i64)> = query_pairs(
        connection,
        "SELECT day, count(*) FROM resources WHERE service = 'api' GROUP BY day ORDER BY day",
    );
    assert_eq!(resources, [(yesterday_name, 1), (today_name.clone(), 1)]);

    let found: Vec<(String, String)> = query_pairs(
        connection,
        &format!(
            "SELECT '{today_name}', body FROM \"{today_name}\".logs_fts WHERE logs_fts MATCH 'payment'"
        ),
    );
    assert_eq!(found, [(today_name, "payment 12 failed".into())]);
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

    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + 86_400 * SECOND).unwrap(),
    )
    .unwrap();
    let connection = reader.connection();
    let bodies: Vec<(String, String)> = query_pairs(connection, "SELECT day, body FROM logs");
    assert_eq!(bodies, [(Day::today().to_string(), "kept".into())]);
    let dropped: Vec<(String, f64)> = query_pairs(
        connection,
        "SELECT series.unit, point.value
         FROM points point
         JOIN series ON series.day = point.day AND series.id = point.series_id
         JOIN resources resource
           ON resource.day = series.day AND resource.id = series.resource_id
         WHERE resource.service = 'otelo'
           AND series.name = 'otelo.telemetry.dropped_batches'",
    );
    assert_eq!(dropped, [("{batch}".into(), 1.0)]);
}

#[test]
fn deletes_the_files_past_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    let expired = Day::today().add_days(-7);
    let kept = Day::today().add_days(-6);
    let names = [
        expired.file_name(),
        format!("{}-wal", expired.file_name()),
        kept.file_name(),
        "notes.txt".into(),
    ];
    for name in &names {
        fs::write(directory.path().join(name), "").unwrap();
    }
    write_batches(directory.path(), Vec::new());
    for name in &names[..2] {
        assert!(
            !directory.path().join(name).exists(),
            "{name} is still there"
        );
    }
    for name in &names[2..] {
        assert!(directory.path().join(name).exists(), "{name} is gone");
    }
}

#[test]
fn skips_records_past_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    let expired = Day::today().add_days(-7);
    write_batches(
        directory.path(),
        vec![log_batch(expired.start_at(), "too old")],
    );
    assert!(!directory.path().join(expired.file_name()).exists());
}

#[test]
fn a_range_without_files_reads_empty_tables() {
    let directory = tempfile::tempdir().unwrap();
    let reader = Reader::open(directory.path(), TimeRange::new(0, SECOND).unwrap()).unwrap();
    assert!(reader.days().is_empty());
    let count: i64 = reader
        .connection()
        .query_row("SELECT count(*) FROM logs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn a_reader_cannot_write() {
    let directory = tempfile::tempdir().unwrap();
    let today_start_at = Day::today().start_at();
    write_batches(directory.path(), vec![log_batch(today_start_at, "kept")]);
    let reader = Reader::open(
        directory.path(),
        TimeRange::new(today_start_at, today_start_at + SECOND).unwrap(),
    )
    .unwrap();
    let day = Day::today();
    let result = reader
        .connection()
        .execute(&format!("DELETE FROM \"{day}\".logs"), []);
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
        "SELECT name || ' ' || kind, temporality FROM series
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
        "SELECT point.histogram ->> '$.counts', point.value
         FROM points point
         JOIN series ON series.day = point.day AND series.id = point.series_id
         WHERE series.name = 'request.duration'",
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
        "SELECT series.name, point.value
         FROM points point
         JOIN series ON series.day = point.day AND series.id = point.series_id
         WHERE series.name = 'process.memory.usage'
         ORDER BY point.recorded_at",
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
        "JOIN series ON series.day = point.day AND series.id = point.series_id
         WHERE series.name = 'cart.adds'";
    assert_eq!(
        query_integer(
            connection,
            "SELECT count(*) FROM series WHERE name = 'cart.adds'"
        ),
        1000
    );
    // A series from before the cap still takes points.
    assert_eq!(
        query_integer(
            connection,
            &format!("SELECT count(*) FROM points point {join_cart_adds_series}")
        ),
        1001
    );
    assert_eq!(
        query_integer(
            connection,
            "SELECT count(*) FROM series WHERE name = 'process.memory.usage'"
        ),
        1
    );
    let rejected_points_counter: Vec<(String, f64)> = query_pairs(
        connection,
        "SELECT series.kind || ' ' || series.unit, point.value
         FROM points point
         JOIN series ON series.day = point.day AND series.id = point.series_id
         WHERE series.name = 'otelo.telemetry.rejected_points'",
    );
    assert_eq!(rejected_points_counter, [("counter {point}".into(), 2.0)]);
}

#[test]
fn the_size_counts_every_file_of_the_telemetry_and_of_the_state() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(directory.path()).unwrap();
    let state_bytes = fs::metadata(directory.path().join("state.sqlite"))
        .unwrap()
        .len();
    assert!(state_bytes > 0);
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes: 0,
            rollup_bytes: 0,
            state_bytes,
        }
    );

    let telemetry_directory = directory.path().join("telemetry");
    fs::create_dir_all(&telemetry_directory).unwrap();
    let day_file_name = Day::today().file_name();
    for (file_name, byte_count) in [
        (day_file_name.clone(), 4096),
        (format!("{day_file_name}-wal"), 512),
        (format!("{day_file_name}-shm"), 32),
        ("metrics-rollup.sqlite".into(), 2048),
        ("metrics-rollup.sqlite-wal".into(), 64),
    ] {
        fs::write(telemetry_directory.join(file_name), vec![0; byte_count]).unwrap();
    }
    fs::write(directory.path().join("state.sqlite-wal"), [0; 7]).unwrap();
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes: 4096 + 512 + 32,
            rollup_bytes: 2048 + 64,
            state_bytes: state_bytes + 7,
        }
    );
}
