use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Map, Value, json};
use siner_telemetry::{
    Batch, Config, Day, Log, Metric, MetricKind, Point, Reader, Records, Resource, Sender, Span,
    Writer, channel,
};

const SECOND: i64 = 1_000_000_000;

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

fn api() -> Resource {
    Resource {
        service: "api".into(),
        attributes: object(&json!({"service.name": "api", "host.name": "droplet"})),
    }
}

fn log(ts: i64, body: &str) -> Log {
    Log {
        ts,
        severity: 9,
        body: body.into(),
        trace_id: Some([1; 16]),
        span_id: Some([2; 8]),
        attributes: object(&json!({"user": 7})),
        source: "otlp",
    }
}

fn span(ts: i64, name: &str) -> Span {
    Span {
        trace_id: [1; 16],
        span_id: [2; 8],
        parent_span_id: None,
        name: name.into(),
        kind: 2,
        start_ts: ts,
        duration_ns: 5_000_000,
        status: 0,
        attributes: Map::new(),
        events: vec![json!({"name": "retry"})],
    }
}

fn memory(points: &[(i64, f64)]) -> Metric {
    Metric {
        name: "process.memory.usage".into(),
        kind: MetricKind::Gauge,
        unit: "By".into(),
        labels: object(&json!({"state": "used"})),
        points: points
            .iter()
            .map(|&(ts, value)| Point {
                ts,
                value,
                histogram: None,
            })
            .collect(),
    }
}

fn logs(ts: i64, body: &str) -> Batch {
    vec![Records {
        resource: api(),
        logs: vec![log(ts, body)],
        spans: Vec::new(),
        metrics: Vec::new(),
    }]
}

/// Starts a writer on `dir`, sends the batches, and waits until it wrote them.
fn write(dir: &Path, batches: Vec<Batch>) {
    let (sender, inbox) = channel(batches.len().max(1));
    for batch in batches {
        assert!(sender.send(batch));
    }
    finish(dir, sender, inbox);
}

fn finish(dir: &Path, sender: Sender, inbox: siner_telemetry::Inbox) {
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
    write(dir.path(), vec![all, logs(after, "user 8 signed in")]);

    let reader = Reader::open(dir.path(), before, after + 1).unwrap();
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

    // Two batches of one resource store it once per day file.
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
    let (sender, inbox) = channel(1);
    assert!(sender.send(logs(now, "kept")));
    assert!(!sender.send(logs(now, "dropped")));
    assert_eq!(sender.dropped(), 1);
    finish(dir.path(), sender, inbox);

    let reader = Reader::open(dir.path(), now, now + 86_400 * SECOND).unwrap();
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
    write(dir.path(), Vec::new());
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
    write(dir.path(), vec![logs(expired.start(), "too old")]);
    assert!(!dir.path().join(expired.file_name()).exists());
}

#[test]
fn a_range_without_files_reads_empty_tables() {
    let dir = tempfile::tempdir().unwrap();
    let reader = Reader::open(dir.path(), 0, SECOND).unwrap();
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
    write(dir.path(), vec![logs(now, "kept")]);
    let reader = Reader::open(dir.path(), now, now + SECOND).unwrap();
    let day = Day::today();
    let result = reader
        .conn()
        .execute(&format!("DELETE FROM \"{day}\".logs"), []);
    assert!(result.is_err());
}
