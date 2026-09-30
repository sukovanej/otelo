use std::fs;
use std::path::Path;

use otelo_storage::{
    Attributes, Batch, Buckets, ExplicitBuckets, Histogram, HistogramPoint, Inbox, Log, Metric,
    NumberPoint, Points, Records, Resource, Sender, Severity, Span, SpanEvent, SpanId, SpanKind,
    SpanStatus, Storage, StorageSize, Temporality, TimeRange, TraceId, batch_channel,
};
use otelo_storage_sqlite::{Config, Day, Reader, Sqlite, Writer};
use rusqlite::Connection;
use serde_json::{Value, json};

fn attributes_from_json(value: Value) -> Attributes {
    serde_json::from_value(value).unwrap()
}

const SECOND: i64 = 1_000_000_000;

fn api() -> Resource {
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
        trace_id: Some(TraceId([1; 16])),
        span_id: Some(SpanId([2; 8])),
        attributes: attributes_from_json(json!({"user": 7})),
        source: "otlp",
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

fn memory(points: &[(i64, f64)]) -> Metric {
    Metric {
        name: "process.memory.usage".into(),
        unit: "By".into(),
        labels: attributes_from_json(json!({"state": "used"})),
        points: Points::UpDown(number_points(points)),
    }
}

fn metrics(metrics: Vec<Metric>) -> Batch {
    vec![Records {
        resource: api(),
        logs: Vec::new(),
        spans: Vec::new(),
        metrics,
    }]
}

fn logs(logged_at: i64, body: &str) -> Batch {
    vec![Records {
        resource: api(),
        logs: vec![log(logged_at, body)],
        spans: Vec::new(),
        metrics: Vec::new(),
    }]
}

fn write_batches(dir: &Path, batches: Vec<Batch>) {
    let (sender, inbox) = batch_channel(batches.len().max(1));
    for batch in batches {
        assert!(sender.send(batch));
    }
    finish(dir, sender, inbox);
}

fn finish(dir: &Path, sender: Sender, inbox: Inbox) {
    let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn rows<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str) -> Vec<(String, T)> {
    conn.prepare(sql)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn reads_back_each_kind_across_a_day_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let today = Day::today();
    let yesterday = today.plus(-1);
    let before = today.start() - SECOND;
    let after = today.start() + SECOND;
    let all = vec![Records {
        resource: api(),
        logs: vec![
            log(before, "user 7 signed in"),
            log(after, "payment 12 failed"),
        ],
        spans: vec![span(before, "GET /languages"), span(after, "POST /matches")],
        metrics: vec![memory(&[(before, 100.0), (after, 200.0)])],
    }];
    write_batches(dir.path(), vec![all, logs(after, "user 8 signed in")]);

    let reader = Reader::open(dir.path(), TimeRange::new(before, after + 1).unwrap()).unwrap();
    assert_eq!(reader.days(), [yesterday, today]);
    let conn = reader.conn();
    let (y, t) = (yesterday.to_string(), today.to_string());

    let bodies: Vec<(String, String)> = rows(conn, "SELECT day, body FROM logs ORDER BY ts, body");
    assert_eq!(
        bodies,
        [
            (y.clone(), "user 7 signed in".into()),
            (t.clone(), "payment 12 failed".into()),
            (t.clone(), "user 8 signed in".into()),
        ]
    );
    let names: Vec<(String, String)> = rows(conn, "SELECT day, name FROM spans ORDER BY start_ts");
    assert_eq!(
        names,
        [
            (y.clone(), "GET /languages".into()),
            (t.clone(), "POST /matches".into())
        ]
    );
    let values: Vec<(String, f64)> = rows(
        conn,
        "SELECT p.day, p.value FROM points p
         JOIN series s ON s.day = p.day AND s.id = p.series_id
         WHERE s.name = 'process.memory.usage' ORDER BY p.ts",
    );
    assert_eq!(values, [(y.clone(), 100.0), (t.clone(), 200.0)]);

    let resources: Vec<(String, i64)> = rows(
        conn,
        "SELECT day, count(*) FROM resources WHERE service = 'api' GROUP BY day ORDER BY day",
    );
    assert_eq!(resources, [(y, 1), (t.clone(), 1)]);

    let found: Vec<(String, String)> = rows(
        conn,
        &format!("SELECT '{t}', body FROM \"{t}\".logs_fts WHERE logs_fts MATCH 'payment'"),
    );
    assert_eq!(found, [(t, "payment 12 failed".into())]);
}

#[test]
fn a_full_channel_drops_the_batch_and_the_writer_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let now = Day::today().start();
    let (sender, inbox) = batch_channel(1);
    assert!(sender.send(logs(now, "kept")));
    assert!(!sender.send(logs(now, "dropped")));
    assert_eq!(sender.dropped_batches(), 1);
    finish(dir.path(), sender, inbox);

    let reader = Reader::open(
        dir.path(),
        TimeRange::new(now, now + 86_400 * SECOND).unwrap(),
    )
    .unwrap();
    let conn = reader.conn();
    let bodies: Vec<(String, String)> = rows(conn, "SELECT day, body FROM logs");
    assert_eq!(bodies, [(Day::today().to_string(), "kept".into())]);
    let dropped: Vec<(String, f64)> = rows(
        conn,
        "SELECT s.unit, p.value FROM points p
         JOIN series s ON s.day = p.day AND s.id = p.series_id
         JOIN resources r ON r.day = s.day AND r.id = s.resource_id
         WHERE r.service = 'otelo' AND s.name = 'otelo.telemetry.dropped_batches'",
    );
    assert_eq!(dropped, [("{batch}".into(), 1.0)]);
}

#[test]
fn deletes_the_files_past_the_retention() {
    let dir = tempfile::tempdir().unwrap();
    let expired = Day::today().plus(-7);
    let kept = Day::today().plus(-6);
    let names = [
        expired.file_name(),
        format!("{}-wal", expired.file_name()),
        format!("{}.schema-0", expired.file_name()),
        kept.file_name(),
        format!("{}.schema-0", kept.file_name()),
        "notes.txt".into(),
    ];
    for name in &names {
        fs::write(dir.path().join(name), "").unwrap();
    }
    write_batches(dir.path(), Vec::new());
    for name in &names[..3] {
        assert!(!dir.path().join(name).exists(), "{name} is still there");
    }
    for name in &names[3..] {
        assert!(dir.path().join(name).exists(), "{name} is gone");
    }
}

#[test]
fn skips_records_past_the_retention() {
    let dir = tempfile::tempdir().unwrap();
    let expired = Day::today().plus(-7);
    write_batches(dir.path(), vec![logs(expired.start(), "too old")]);
    assert!(!dir.path().join(expired.file_name()).exists());
}

#[test]
fn a_range_without_files_reads_empty_tables() {
    let dir = tempfile::tempdir().unwrap();
    let reader = Reader::open(dir.path(), TimeRange::new(0, SECOND).unwrap()).unwrap();
    assert!(reader.days().is_empty());
    let count: i64 = reader
        .conn()
        .query_row("SELECT count(*) FROM logs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn a_reader_cannot_write() {
    let dir = tempfile::tempdir().unwrap();
    let now = Day::today().start();
    write_batches(dir.path(), vec![logs(now, "kept")]);
    let reader = Reader::open(dir.path(), TimeRange::new(now, now + SECOND).unwrap()).unwrap();
    let day = Day::today();
    let result = reader
        .conn()
        .execute(&format!("DELETE FROM \"{day}\".logs"), []);
    assert!(result.is_err());
}

fn todays_reader(dir: &Path) -> Reader {
    let start = Day::today().start();
    Reader::open(dir, TimeRange::new(start, start + 86_400 * SECOND).unwrap()).unwrap()
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn stores_the_kind_and_the_temporality_of_each_series() {
    let dir = tempfile::tempdir().unwrap();
    let now = Day::today().start();
    let metric = |name: &str, points: Points| Metric {
        name: name.into(),
        unit: "1".into(),
        labels: Attributes::new(),
        points,
    };
    let one = || number_points(&[(now, 1.0)]);
    let duration = HistogramPoint {
        recorded_at: now,
        histogram: Histogram {
            count: 3,
            sum: Some(2.5),
            min: None,
            max: None,
            buckets: Buckets::Explicit(ExplicitBuckets {
                bounds: vec![1.0],
                counts: vec![2, 1],
            }),
        },
    };
    write_batches(
        dir.path(),
        vec![metrics(vec![
            metric("queue.lag", Points::Gauge(one())),
            metric("memory.used", Points::UpDown(one())),
            metric(
                "emails.sent",
                Points::Counter(Temporality::Cumulative, one()),
            ),
            metric("bytes.sent", Points::Counter(Temporality::Delta, one())),
            metric(
                "request.duration",
                Points::Histogram(Temporality::Delta, vec![duration]),
            ),
        ])],
    );
    let reader = todays_reader(dir.path());
    let kinds: Vec<(String, Option<String>)> = rows(
        reader.conn(),
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
    let sums: Vec<(String, f64)> = rows(
        reader.conn(),
        "SELECT p.histogram ->> '$.counts', p.value FROM points p
         JOIN series s ON s.day = p.day AND s.id = p.series_id
         WHERE s.name = 'request.duration'",
    );
    assert_eq!(sums, [("[2,1]".into(), 2.5)]);
}

#[test]
fn a_batch_written_twice_leaves_each_point_once() {
    let dir = tempfile::tempdir().unwrap();
    let now = Day::today().start();
    write_batches(
        dir.path(),
        vec![
            metrics(vec![memory(&[(now, 100.0), (now + SECOND, 200.0)])]),
            metrics(vec![memory(&[(now, 150.0), (now + SECOND, 200.0)])]),
        ],
    );
    let reader = todays_reader(dir.path());
    let values: Vec<(String, f64)> = rows(
        reader.conn(),
        "SELECT s.name, p.value FROM points p
         JOIN series s ON s.day = p.day AND s.id = p.series_id
         WHERE s.name = 'process.memory.usage' ORDER BY p.ts",
    );
    let name = || String::from("process.memory.usage");
    assert_eq!(values, [(name(), 150.0), (name(), 200.0)]);
}

#[test]
fn a_metric_past_1000_series_rejects_the_points_of_its_newer_series() {
    let dir = tempfile::tempdir().unwrap();
    let now = Day::today().start();
    let cart_adds_of_user = |user: i64, recorded_at: i64| Metric {
        name: "cart.adds".into(),
        unit: "{item}".into(),
        labels: attributes_from_json(json!({"user.id": user})),
        points: Points::Gauge(number_points(&[(recorded_at, 1.0)])),
    };
    write_batches(
        dir.path(),
        vec![
            metrics((0..1001).map(|user| cart_adds_of_user(user, now)).collect()),
            metrics(vec![
                cart_adds_of_user(0, now + SECOND),
                cart_adds_of_user(2000, now + SECOND),
                memory(&[(now, 1.0)]),
            ]),
        ],
    );
    let reader = todays_reader(dir.path());
    let conn = reader.conn();
    let of_cart_adds = "JOIN series s ON s.day = p.day AND s.id = p.series_id
                        WHERE s.name = 'cart.adds'";
    assert_eq!(
        count(conn, "SELECT count(*) FROM series WHERE name = 'cart.adds'"),
        1000
    );
    // A series from before the cap still takes points.
    assert_eq!(
        count(
            conn,
            &format!("SELECT count(*) FROM points p {of_cart_adds}")
        ),
        1001
    );
    assert_eq!(
        count(
            conn,
            "SELECT count(*) FROM series WHERE name = 'process.memory.usage'"
        ),
        1
    );
    let rejected: Vec<(String, f64)> = rows(
        conn,
        "SELECT s.kind || ' ' || s.unit, p.value FROM points p
         JOIN series s ON s.day = p.day AND s.id = p.series_id
         WHERE s.name = 'otelo.telemetry.rejected_points'",
    );
    assert_eq!(rejected, [("counter {point}".into(), 2.0)]);
}

#[test]
fn sets_aside_a_day_file_of_another_schema() {
    let dir = tempfile::tempdir().unwrap();
    let today = Day::today();
    let path = dir.path().join(today.file_name());
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE points (series_id INTEGER, ts INTEGER, value REAL, histogram TEXT);
             INSERT INTO points VALUES (1, 2, 3.0, NULL);",
        )
        .unwrap();
    write_batches(
        dir.path(),
        vec![metrics(vec![memory(&[(today.start(), 100.0)])])],
    );

    let aside = dir.path().join(format!("{}.schema-0", today.file_name()));
    let old = Connection::open(&aside).unwrap();
    assert_eq!(
        count(&old, "SELECT count(*) FROM points WHERE value = 3.0"),
        1
    );
    let new = Connection::open(&path).unwrap();
    assert_eq!(count(&new, "PRAGMA user_version"), 1);
    let reader = todays_reader(dir.path());
    assert_eq!(
        count(
            reader.conn(),
            "SELECT count(*) FROM points p
             JOIN series s ON s.day = p.day AND s.id = p.series_id
             WHERE s.name = 'process.memory.usage' AND p.value = 100.0"
        ),
        1
    );
}

#[test]
fn the_size_counts_every_file_of_the_telemetry_and_of_the_state() {
    let dir = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(dir.path()).unwrap();
    let state_bytes = fs::metadata(dir.path().join("state.sqlite")).unwrap().len();
    assert!(state_bytes > 0);
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes: 0,
            rollup_bytes: 0,
            state_bytes,
        }
    );

    let telemetry = dir.path().join("telemetry");
    fs::create_dir_all(&telemetry).unwrap();
    let day = Day::today().file_name();
    for (name, bytes) in [
        (day.clone(), 4096),
        (format!("{day}-wal"), 512),
        (format!("{day}-shm"), 32),
        (format!("{day}.schema-0"), 100),
        ("metrics-rollup.sqlite".into(), 2048),
        ("metrics-rollup.sqlite-wal".into(), 64),
    ] {
        fs::write(telemetry.join(name), vec![0; bytes]).unwrap();
    }
    fs::write(dir.path().join("state.sqlite-wal"), [0; 7]).unwrap();
    assert_eq!(
        storage.size().unwrap(),
        StorageSize {
            telemetry_bytes: 4096 + 512 + 32 + 100,
            rollup_bytes: 2048 + 64,
            state_bytes: state_bytes + 7,
        }
    );
}
