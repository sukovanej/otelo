mod common;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use otelo_indexed_storage::{
    Attributes, Batch, FrameCounts, Log, Metric, NumberPoint, PipelineMeters, Points, Records,
    Resource, Severity, TraceContext, now_unix_nanos,
};
use otelo_indexed_storage_sqlite::{
    Config, Day, FrameMapper, Indexer, Progress, TELEMETRY_FILE_NAME, TelemetryFile,
    index_journal_until_caught_up,
};
use otelo_journal::Journal;
use otelo_query::Signal;
use rusqlite::Connection;

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const NANOS_PER_HOUR: i64 = 3_600 * NANOS_PER_SECOND;
const NANOS_PER_DAY: i64 = 24 * NANOS_PER_HOUR;

fn numbered_log_batches(count: u32) -> Vec<Batch> {
    let logged_at = Day::today().start_at();
    (0..count)
        .map(|number| {
            vec![Records {
                resource: Resource {
                    service: "api".into(),
                    attributes: Attributes::new(),
                },
                logs: vec![Log {
                    logged_at,
                    severity_number: Severity::INFO,
                    body: format!("frame {number}"),
                    trace_context: TraceContext::None,
                    attributes: Attributes::new(),
                }],
                spans: Vec::new(),
                metrics: Vec::new(),
            }]
        })
        .collect()
}

fn received_now(count: u32) -> Vec<(u32, i64)> {
    (0..count)
        .map(|number| (number, now_unix_nanos()))
        .collect()
}

fn read_log_bodies(telemetry_directory: &Path) -> Vec<String> {
    let path = telemetry_directory.join(TELEMETRY_FILE_NAME);
    if !path.exists() {
        return Vec::new();
    }
    Connection::open(path)
        .unwrap()
        .prepare("SELECT body FROM logs ORDER BY rowid")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn an_indexer_that_stopped_mid_transaction_resumes_with_every_frame_once() {
    let directory = tempfile::tempdir().unwrap();
    let telemetry_directory = directory.path().join("telemetry");
    let opened = common::open_journal(directory.path());
    common::append_numbered_frames(opened.journal.as_ref(), Signal::Logs, &received_now(150));
    let batches = numbered_log_batches(150);

    let crashing_batches = batches.clone();
    let crashing_map_frame: FrameMapper = Arc::new(move |_, request| {
        let number = u32::from_le_bytes(request.try_into()?);
        assert!(number != 100, "the indexer stops in its second transaction");
        Ok(crashing_batches[number as usize].clone())
    });
    let crashed = std::thread::spawn({
        let config = Config::new(telemetry_directory.clone());
        let journal = Arc::clone(&opened.journal);
        move || {
            index_journal_until_caught_up(
                config,
                journal,
                crashing_map_frame,
                Arc::new(PipelineMeters::default()),
                |_, _| {},
            )
        }
    })
    .join();
    assert!(crashed.is_err());
    assert_eq!(read_log_bodies(&telemetry_directory).len(), 64);

    index_journal_until_caught_up(
        Config::new(telemetry_directory.clone()),
        Arc::clone(&opened.journal) as _,
        common::map_numbered_frames(batches),
        Arc::new(PipelineMeters::default()),
        |_, _| {},
    )
    .unwrap();
    let expected: Vec<String> = (0..150).map(|number| format!("frame {number}")).collect();
    assert_eq!(read_log_bodies(&telemetry_directory), expected);
    opened.threads.stop_and_join().unwrap();
}

#[test]
fn skips_the_frames_received_before_the_retention() {
    let directory = tempfile::tempdir().unwrap();
    let telemetry_directory = directory.path().join("telemetry");
    let opened = common::open_journal(directory.path());
    let now = now_unix_nanos();
    common::append_numbered_frames(
        opened.journal.as_ref(),
        Signal::Logs,
        &[
            (0, now - 8 * NANOS_PER_DAY),
            (1, now),
            (2, now - 8 * NANOS_PER_DAY),
        ],
    );
    let meters = Arc::new(PipelineMeters::default());
    index_journal_until_caught_up(
        Config::new(telemetry_directory.clone()),
        Arc::clone(&opened.journal) as _,
        common::map_numbered_frames(numbered_log_batches(3)),
        Arc::clone(&meters),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(read_log_bodies(&telemetry_directory), ["frame 1"]);
    assert_eq!(
        meters.read_pipeline().logs.frames,
        FrameCounts {
            indexed: 1,
            skipped: 1,
            undecodable: 0,
        }
    );
    opened.threads.stop_and_join().unwrap();
}

#[test]
fn skips_a_frame_that_does_not_decode() {
    let directory = tempfile::tempdir().unwrap();
    let telemetry_directory = directory.path().join("telemetry");
    let opened = common::open_journal(directory.path());
    // The sync of the next frame covers this one.
    drop(
        opened
            .journal
            .append_frame(Signal::Logs, now_unix_nanos(), b"not a number")
            .unwrap(),
    );
    common::append_numbered_frames(opened.journal.as_ref(), Signal::Logs, &received_now(1));
    let meters = Arc::new(PipelineMeters::default());
    index_journal_until_caught_up(
        Config::new(telemetry_directory.clone()),
        Arc::clone(&opened.journal) as _,
        common::map_numbered_frames(numbered_log_batches(1)),
        Arc::clone(&meters),
        |_, _| {},
    )
    .unwrap();
    assert_eq!(read_log_bodies(&telemetry_directory), ["frame 0"]);
    assert_eq!(meters.read_pipeline().logs.frames.undecodable, 1);
    opened.threads.stop_and_join().unwrap();
}

#[test]
fn indexes_each_frame_once_the_journal_synced_it() {
    let directory = tempfile::tempdir().unwrap();
    let telemetry_directory = directory.path().join("telemetry");
    let opened = common::open_journal(directory.path());
    let meters = Arc::new(PipelineMeters::default());
    let indexer = Indexer::spawn(
        Config::new(telemetry_directory.clone()),
        Arc::clone(&opened.journal) as _,
        opened.synced_ends,
        common::map_numbered_frames(numbered_log_batches(3)),
        Arc::clone(&meters),
    )
    .unwrap();

    common::append_numbered_frames(opened.journal.as_ref(), Signal::Logs, &received_now(2));
    let deadline = Instant::now() + Duration::from_secs(5);
    while read_log_bodies(&telemetry_directory).len() < 2 {
        assert!(
            Instant::now() < deadline,
            "the indexer never indexed the frames"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    common::append_numbered_frames(
        opened.journal.as_ref(),
        Signal::Logs,
        &[(2, now_unix_nanos())],
    );
    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();

    assert_eq!(
        read_log_bodies(&telemetry_directory),
        ["frame 0", "frame 1", "frame 2"]
    );
    let logs_reading = meters.read_pipeline().logs;
    assert_eq!(logs_reading.frames.indexed, 3);
    assert_eq!(logs_reading.records.written, 3);
    assert_eq!(logs_reading.index_lag, Duration::ZERO);
    assert!(logs_reading.index_transactions.count() >= 2);
}

fn gauge_batch(recorded_at: i64) -> Batch {
    vec![Records {
        resource: Resource {
            service: "api".into(),
            attributes: Attributes::new(),
        },
        logs: Vec::new(),
        spans: Vec::new(),
        metrics: vec![Metric {
            name: "queue.depth".into(),
            unit: "1".into(),
            attributes: Attributes::new(),
            points: Points::Gauge(vec![NumberPoint {
                recorded_at,
                value: 1.0,
            }]),
        }],
    }]
}

fn count_summaries(telemetry_directory: &Path, table: &str) -> i64 {
    Connection::open(telemetry_directory.join(TELEMETRY_FILE_NAME))
        .unwrap()
        .query_row(
            &format!(
                "SELECT count(*)
                 FROM {table} summary
                 JOIN metric_series ON metric_series.id = summary.metric_series_id
                 WHERE metric_series.name = ?1"
            ),
            ["queue.depth"],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn rolls_up_only_the_minutes_it_has_indexed_while_it_catches_up() {
    let directory = tempfile::tempdir().unwrap();
    let telemetry_directory = directory.path().join("telemetry");
    let opened = common::open_journal(directory.path());
    let first_recorded_at = (now_unix_nanos().div_euclid(NANOS_PER_HOUR) - 3) * NANOS_PER_HOUR;
    let recorded_ats: Vec<i64> = (0..720)
        .map(|number| first_recorded_at + number * 10 * NANOS_PER_SECOND)
        .collect();
    let numbered_received_ats: Vec<(u32, i64)> = (0..).zip(recorded_ats.iter().copied()).collect();
    common::append_numbered_frames(
        opened.journal.as_ref(),
        Signal::Metrics,
        &numbered_received_ats,
    );
    let indexer = Indexer::spawn(
        Config::new(telemetry_directory.clone()),
        Arc::clone(&opened.journal) as _,
        opened.synced_ends,
        common::map_numbered_frames(recorded_ats.into_iter().map(gauge_batch).collect()),
        Arc::new(PipelineMeters::default()),
    )
    .unwrap();
    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();

    let mut file = TelemetryFile::open(&telemetry_directory).unwrap();
    let oldest_point_at = Day::today().add_days(-6).start_at();
    while file
        .roll_up_next_due(now_unix_nanos(), oldest_point_at)
        .unwrap()
        == Progress::MoreIsDue
    {}
    assert_eq!(
        count_summaries(&telemetry_directory, "metric_minute_summaries"),
        120
    );
    assert_eq!(
        count_summaries(&telemetry_directory, "metric_hour_summaries"),
        2
    );
}
