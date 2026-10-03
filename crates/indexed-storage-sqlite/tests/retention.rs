use std::fs;
use std::num::NonZeroU16;
use std::path::Path;

use otelo_indexed_storage::{
    Attributes, Batch, Log, Metric, NumberPoint, Points, RangeQueries, Records, Resource, Severity,
    Span, SpanId, SpanKind, SpanStatus, TimeRange, TraceContext, TraceId, open_batch_channel,
};
use otelo_indexed_storage_sqlite::{
    Config, Day, Progress, Reader, TELEMETRY_FILE_NAME, TelemetryFile, Writer,
};
use otelo_query::{Catalog, Signal, parse_query};
use rusqlite::Connection;
use serde_json::json;

const HOUR: i64 = 3600 * 1_000_000_000;

fn three_days_ago() -> i64 {
    Day::today().add_days(-3).start_at() + HOUR
}

fn now() -> i64 {
    otelo_indexed_storage::now_unix_nanos()
}

fn resource(service: &str) -> Resource {
    Resource {
        service: service.into(),
        attributes: serde_json::from_value(json!({"service.name": service})).unwrap(),
    }
}

fn log(logged_at: i64, body: &str, attributes: serde_json::Value) -> Log {
    Log {
        logged_at,
        severity: Severity::INFO,
        body: body.into(),
        trace_context: TraceContext::None,
        attributes: serde_json::from_value(attributes).unwrap(),
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
        duration_ns: 1_000,
        status: SpanStatus::Unset,
        attributes: Attributes::new(),
        events: Vec::new(),
    }
}

fn gauge(name: &str, recorded_ats: &[i64]) -> Metric {
    Metric {
        name: name.into(),
        unit: "1".into(),
        attributes: Attributes::new(),
        points: Points::Gauge(
            recorded_ats
                .iter()
                .map(|&recorded_at| NumberPoint {
                    recorded_at,
                    value: 1.0,
                })
                .collect(),
        ),
    }
}

fn records(service: &str, logs: Vec<Log>, spans: Vec<Span>, metrics: Vec<Metric>) -> Batch {
    vec![Records {
        resource: resource(service),
        logs,
        spans,
        metrics,
    }]
}

const fn days(count: u16) -> NonZeroU16 {
    NonZeroU16::new(count).unwrap()
}

// The writer deletes what is past the retention before it takes a batch.
fn run_writer(directory: &Path, batches: Vec<Batch>, configure: impl FnOnce(&mut Config)) {
    let (sender, inbox) = open_batch_channel(batches.len().max(1));
    for batch in batches {
        assert!(sender.send_batch(batch));
    }
    let mut config = Config::new(directory.to_owned());
    configure(&mut config);
    let writer = Writer::spawn(config, inbox).unwrap();
    drop(sender);
    writer.join().unwrap();
}

fn open_telemetry_file(directory: &Path) -> Connection {
    Connection::open(directory.join(TELEMETRY_FILE_NAME)).unwrap()
}

fn count_rows(directory: &Path, sql: &str) -> i64 {
    open_telemetry_file(directory)
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

fn reader_of_the_week(directory: &Path) -> Reader {
    let range = TimeRange::new(Day::today().add_days(-6).start_at(), now() + HOUR).unwrap();
    Reader::open(directory, range).unwrap()
}

#[test]
fn deletes_the_rows_of_one_signal_and_leaves_the_others() {
    let directory = tempfile::tempdir().unwrap();
    let at = three_days_ago();
    run_writer(
        directory.path(),
        vec![records(
            "api",
            vec![log(at, "signed in", json!({}))],
            vec![span(at, "GET /")],
            vec![gauge("queue.lag", &[at])],
        )],
        |_| {},
    );
    run_writer(directory.path(), Vec::new(), |config| {
        config.logs_retention_days = days(2);
    });
    assert_eq!(count_rows(directory.path(), "SELECT count(*) FROM logs"), 0);
    assert_eq!(
        count_rows(directory.path(), "SELECT count(*) FROM spans"),
        1
    );
    assert_eq!(
        count_rows(
            directory.path(),
            "SELECT count(*) FROM metric_points point
             JOIN metric_series ON metric_series.id = point.metric_series_id
             WHERE metric_series.name = 'queue.lag'"
        ),
        1
    );
}

#[test]
fn a_deleted_log_no_longer_matches_a_full_text_search() {
    let directory = tempfile::tempdir().unwrap();
    run_writer(
        directory.path(),
        vec![records(
            "api",
            vec![
                log(three_days_ago(), "payment 12 failed", json!({})),
                log(now(), "payment 13 failed", json!({})),
            ],
            Vec::new(),
            Vec::new(),
        )],
        |_| {},
    );
    let search_bodies = || {
        let query = parse_query(r#"body ~ "payment""#, Signal::Logs).unwrap();
        reader_of_the_week(directory.path())
            .list_logs(&query, 10)
            .unwrap()
            .logs
            .into_iter()
            .map(|line| line.body)
            .collect::<Vec<_>>()
    };
    assert_eq!(search_bodies(), ["payment 13 failed", "payment 12 failed"]);
    run_writer(directory.path(), Vec::new(), |config| {
        config.logs_retention_days = days(2);
    });
    assert_eq!(search_bodies(), ["payment 13 failed"]);
    assert_eq!(
        count_rows(
            directory.path(),
            "SELECT count(*) FROM log_body_search WHERE log_body_search MATCH 'payment'"
        ),
        1
    );
}

#[test]
fn the_catalog_drops_a_key_whose_last_day_passed_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    run_writer(
        directory.path(),
        vec![records(
            "api",
            vec![
                log(
                    three_days_ago(),
                    "old",
                    json!({"old.key": 1, "kept.key": 1}),
                ),
                log(now(), "new", json!({"kept.key": 2})),
            ],
            Vec::new(),
            Vec::new(),
        )],
        |_| {},
    );
    let log_keys = || {
        let mut keys: Vec<(String, u64)> = reader_of_the_week(directory.path())
            .keys(Signal::Logs, false)
            .into_iter()
            .map(|key_info| (key_info.key, key_info.count))
            .collect();
        keys.sort();
        keys
    };
    assert_eq!(log_keys(), [("kept.key".into(), 2), ("old.key".into(), 1)]);
    run_writer(directory.path(), Vec::new(), |config| {
        config.logs_retention_days = days(2);
    });
    assert_eq!(log_keys(), [("kept.key".into(), 1)]);
}

#[test]
fn retention_of_the_metrics_deletes_a_series_with_no_rows_left() {
    let directory = tempfile::tempdir().unwrap();
    let at = three_days_ago();
    run_writer(
        directory.path(),
        vec![
            records(
                "api",
                Vec::new(),
                Vec::new(),
                vec![gauge("queue.lag", &[at, now()])],
            ),
            records(
                "worker",
                Vec::new(),
                Vec::new(),
                vec![gauge("jobs.done", &[at])],
            ),
        ],
        |_| {},
    );
    let mut file = TelemetryFile::open(directory.path()).unwrap();
    while file
        .roll_up_next_due(now(), Day::today().add_days(-6).start_at())
        .unwrap()
        == Progress::MoreIsDue
    {}
    drop(file);
    let oldest_retained_at = Day::today().add_days(-1).start_at();
    let count_rows_before_retention = |table: &str, instant_column: &str| -> i64 {
        open_telemetry_file(directory.path())
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE {instant_column} < ?1"),
                [oldest_retained_at],
                |row| row.get(0),
            )
            .unwrap()
    };
    let tables = [
        ("metric_points", "recorded_at"),
        ("metric_minute_summaries", "start_at"),
        ("metric_hour_summaries", "start_at"),
    ];
    for (table, instant_column) in tables {
        assert!(
            count_rows_before_retention(table, instant_column) > 0,
            "{table}"
        );
    }

    run_writer(directory.path(), Vec::new(), |config| {
        config.metrics_retention_days = days(2);
    });
    for (table, instant_column) in tables {
        assert_eq!(
            count_rows_before_retention(table, instant_column),
            0,
            "{table}"
        );
    }
    let connection = open_telemetry_file(directory.path());
    let series_names: Vec<String> = connection
        .prepare("SELECT name FROM metric_series WHERE name NOT LIKE 'otelo.%' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(series_names, ["queue.lag"]);
    let services: Vec<String> = connection
        .prepare("SELECT service FROM resources ORDER BY service")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(services, ["api", "otelo"]);
}

#[test]
fn lowering_a_retention_shrinks_the_file_after_the_incremental_vacuum() {
    let directory = tempfile::tempdir().unwrap();
    let at = three_days_ago();
    // More logs than one transaction of the retention deletes.
    let logs = (0..25_000)
        .map(|index| {
            log(
                at + index,
                &format!("line {index} {}", "x".repeat(200)),
                json!({}),
            )
        })
        .collect();
    run_writer(
        directory.path(),
        vec![records("api", logs, Vec::new(), Vec::new())],
        |_| {},
    );
    let file_bytes = || {
        fs::metadata(directory.path().join(TELEMETRY_FILE_NAME))
            .unwrap()
            .len()
    };
    let bytes_before = file_bytes();
    assert!(bytes_before > 5_000_000, "{bytes_before} bytes");

    run_writer(directory.path(), Vec::new(), |config| {
        config.logs_retention_days = days(2);
    });
    assert_eq!(count_rows(directory.path(), "SELECT count(*) FROM logs"), 0);
    let bytes_after = file_bytes();
    assert!(
        bytes_after < bytes_before / 2,
        "{bytes_before} bytes before, {bytes_after} after"
    );
}
