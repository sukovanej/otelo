use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Value, json};
use siner_storage::{
    Attributes, Batch, Inbox, Log, Metric, MetricKind, Point, Records, Resource, Sender, Severity,
    Span, SpanEvent, SpanId, SpanKind, SpanStatus, TimeRange, TraceId, batch_channel,
};
use siner_storage_sqlite::{Config, Day, Reader, Writer};

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

fn memory(points: &[(i64, f64)]) -> Metric {
    Metric {
        name: "process.memory.usage".into(),
        kind: MetricKind::Gauge,
        unit: "By".into(),
        labels: attributes_from_json(json!({"state": "used"})),
        points: points
            .iter()
            .map(|&(recorded_at, value)| Point {
                recorded_at,
                value,
                histogram: None,
            })
            .collect(),
    }
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
         WHERE r.service = 'siner' AND s.name = 'siner.telemetry.dropped_batches'",
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
        kept.file_name(),
        "notes.txt".into(),
    ];
    for name in &names {
        fs::write(dir.path().join(name), "").unwrap();
    }
    write_batches(dir.path(), Vec::new());
    for name in &names[..2] {
        assert!(!dir.path().join(name).exists(), "{name} is still there");
    }
    for name in &names[2..] {
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
