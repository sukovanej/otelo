mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use otelo_indexed_storage::query::{PageRequest, Resolution, SpanSort};
use otelo_indexed_storage::{
    Attributes, Batch, IndexedAttribute, IndexedSignal, Log, Metric, NumberPoint, PipelineMeters,
    Points, RangeQueries, Records, Resource, Severity, Span, SpanId, SpanKind, SpanStatus,
    TimeRange, TraceContext, TraceId, now_unix_nanos,
};
use otelo_indexed_storage_sqlite::{
    Day, ReindexProgress, STORAGE_VERSION, Sqlite, TELEMETRY_FILE_NAME, TelemetryLock,
    reindex_from_journal,
};
use otelo_journal::Hour;
use otelo_query::{Signal, parse_query};
use rusqlite::Connection;

const SECOND: i64 = 1_000_000_000;

fn shop_resource() -> Resource {
    Resource {
        service: "shop".into(),
        attributes: serde_json::from_value(serde_json::json!({"service.name": "shop"})).unwrap(),
    }
}

fn signal_batches() -> [(Signal, Batch); 3] {
    let started_at = Day::today().start_at() + SECOND;
    let records = |logs, spans, metrics| {
        vec![Records {
            resource: shop_resource(),
            logs,
            spans,
            metrics,
        }]
    };
    [
        (
            Signal::Logs,
            records(
                vec![Log {
                    logged_at: started_at,
                    severity_number: Severity::ERROR,
                    body: "payment 12 failed".into(),
                    trace_context: TraceContext::None,
                    attributes: serde_json::from_value(serde_json::json!({"user.id": 7})).unwrap(),
                }],
                Vec::new(),
                Vec::new(),
            ),
        ),
        (
            Signal::Spans,
            records(
                Vec::new(),
                vec![Span {
                    trace_id: TraceId([1; 16]),
                    span_id: SpanId([2; 8]),
                    parent_span_id: None,
                    name: "POST /pay".into(),
                    kind: SpanKind::Server,
                    started_at,
                    duration_ns: 30_000_000,
                    status_code: SpanStatus::Error,
                    attributes: Attributes::new(),
                    events: Vec::new(),
                }],
                Vec::new(),
            ),
        ),
        (
            Signal::Metrics,
            records(
                Vec::new(),
                Vec::new(),
                vec![Metric {
                    name: "queue.depth".into(),
                    unit: "{job}".into(),
                    attributes: Attributes::new(),
                    points: Points::Gauge(vec![NumberPoint {
                        recorded_at: started_at,
                        value: 4.0,
                    }]),
                }],
            ),
        ),
    ]
}

fn append_signal_batches(journal: &dyn otelo_journal::Journal) -> Vec<Batch> {
    let mut batches = Vec::new();
    for (signal, batch) in signal_batches() {
        let number = u32::try_from(batches.len()).unwrap();
        common::append_numbered_frames(journal, signal, &[(number, now_unix_nanos())]);
        batches.push(batch);
    }
    batches
}

fn answer_queries(data_directory: &Path) -> String {
    let today_start_at = Day::today().start_at();
    let reader = otelo_indexed_storage_sqlite::Reader::open(
        &data_directory.join("telemetry"),
        TimeRange::new(today_start_at, today_start_at + 86_400 * SECOND).unwrap(),
    )
    .unwrap();
    format!(
        "{:?}\n{:?}\n{:?}",
        reader
            .list_logs(
                &parse_query("user.id = 7", Signal::Logs).unwrap(),
                &PageRequest::first(10)
            )
            .unwrap(),
        reader
            .list_spans(
                &parse_query("", Signal::Spans).unwrap(),
                SpanSort::Newest,
                &PageRequest::first(10)
            )
            .unwrap(),
        reader
            .list_metrics(
                &parse_query("", Signal::Metrics).unwrap(),
                Resolution::Raw,
                10
            )
            .unwrap(),
    )
}

#[test]
fn a_reindex_answers_the_queries_as_the_live_indexing_did() {
    let journal_directory = tempfile::tempdir().unwrap();
    let opened = common::open_journal(journal_directory.path());
    let live_directory = tempfile::tempdir().unwrap();
    let user_id = IndexedAttribute::new(IndexedSignal::Logs, "user.id").unwrap();
    let indexed_attributes = BTreeSet::from([user_id]);
    let storage = Sqlite::open(
        live_directory.path(),
        common::INDEX_RETENTION_DAYS,
        indexed_attributes.clone(),
    )
    .unwrap();
    let batches = append_signal_batches(opened.journal.as_ref());
    let indexer = storage
        .spawn_indexer(
            Arc::clone(&opened.journal) as _,
            opened.synced_ends,
            common::map_numbered_frames(batches.clone()),
            Arc::new(PipelineMeters::default()),
        )
        .unwrap();
    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();
    drop(storage);

    let reindexed_directory = tempfile::tempdir().unwrap();
    drop(
        Sqlite::open(
            reindexed_directory.path(),
            common::INDEX_RETENTION_DAYS,
            BTreeSet::new(),
        )
        .unwrap(),
    );
    let telemetry_path = reindexed_directory
        .path()
        .join("telemetry")
        .join(TELEMETRY_FILE_NAME);
    Connection::open(&telemetry_path)
        .unwrap()
        .pragma_update(None, "user_version", 0)
        .unwrap();
    let telemetry_lock = TelemetryLock::acquire(reindexed_directory.path()).unwrap();
    let mut progress = Vec::new();
    reindex_from_journal(
        &telemetry_lock,
        common::INDEX_RETENTION_DAYS,
        indexed_attributes,
        Arc::clone(&opened.journal) as _,
        common::map_numbered_frames(batches),
        |step| progress.push(step),
    )
    .unwrap();

    let this_hour = Hour::containing(now_unix_nanos());
    assert_eq!(
        progress,
        [
            ReindexProgress::DeletedOtherStorageVersion { found_version: 0 },
            ReindexProgress::IndexedHour {
                signal: Signal::Logs,
                hour: this_hour
            },
            ReindexProgress::IndexedHour {
                signal: Signal::Spans,
                hour: this_hour
            },
            ReindexProgress::IndexedHour {
                signal: Signal::Metrics,
                hour: this_hour
            },
        ]
    );
    let live_answers = answer_queries(live_directory.path());
    assert!(live_answers.contains("payment 12 failed"), "{live_answers}");
    assert_eq!(answer_queries(reindexed_directory.path()), live_answers);
    let attribute_indexes: i64 = Connection::open(&telemetry_path)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM sqlite_master
             WHERE name GLOB 'logs_interned_attribute_*' OR name GLOB 'logs_literal_attribute_*'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(attribute_indexes, 2);
}

fn index_journal_live(
    data_directory: &Path,
    journal_directory: &Path,
    indexed_attributes: BTreeSet<IndexedAttribute>,
    batches: Vec<Batch>,
) {
    let storage = Sqlite::open(
        data_directory,
        common::INDEX_RETENTION_DAYS,
        indexed_attributes,
    )
    .unwrap();
    let opened = common::open_journal(journal_directory);
    let indexer = storage
        .spawn_indexer(
            Arc::clone(&opened.journal) as _,
            opened.synced_ends,
            common::map_numbered_frames(batches),
            Arc::new(PipelineMeters::default()),
        )
        .unwrap();
    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();
}

#[test]
fn the_storage_of_the_daemon_builds_an_index_of_another_storage_version_again() {
    let journal_directory = tempfile::tempdir().unwrap();
    let opened = common::open_journal(journal_directory.path());
    let batches = append_signal_batches(opened.journal.as_ref());
    opened.threads.stop_and_join().unwrap();
    let user_id = IndexedAttribute::new(IndexedSignal::Logs, "user.id").unwrap();
    let indexed_attributes = BTreeSet::from([user_id]);
    let live_directory = tempfile::tempdir().unwrap();
    index_journal_live(
        live_directory.path(),
        journal_directory.path(),
        indexed_attributes.clone(),
        batches.clone(),
    );

    let rebuilt_directory = tempfile::tempdir().unwrap();
    drop(
        Sqlite::open(
            rebuilt_directory.path(),
            common::INDEX_RETENTION_DAYS,
            BTreeSet::new(),
        )
        .unwrap(),
    );
    let telemetry_path = rebuilt_directory
        .path()
        .join("telemetry")
        .join(TELEMETRY_FILE_NAME);
    let connection = Connection::open(&telemetry_path).unwrap();
    connection
        .execute_batch("CREATE TABLE left_by_another_version (body TEXT)")
        .unwrap();
    connection
        .pragma_update(None, "user_version", STORAGE_VERSION + 1)
        .unwrap();
    drop(connection);
    index_journal_live(
        rebuilt_directory.path(),
        journal_directory.path(),
        indexed_attributes,
        batches,
    );

    let connection = Connection::open(&telemetry_path).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, STORAGE_VERSION);
    let left_table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'left_by_another_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(left_table_count, 0);
    let live_answers = answer_queries(live_directory.path());
    assert!(live_answers.contains("payment 12 failed"), "{live_answers}");
    assert_eq!(answer_queries(rebuilt_directory.path()), live_answers);
}

#[test]
fn a_reindex_refuses_to_run_next_to_the_daemon() {
    let directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(
        directory.path(),
        common::INDEX_RETENTION_DAYS,
        BTreeSet::new(),
    )
    .unwrap();
    let error = TelemetryLock::acquire(directory.path()).err().unwrap();
    assert!(
        error.to_string().starts_with("another otelo holds "),
        "{error:#}"
    );
    drop(storage);
    TelemetryLock::acquire(directory.path()).unwrap();
}
