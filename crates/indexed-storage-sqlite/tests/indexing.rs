mod common;

use std::collections::BTreeSet;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use otelo_indexed_storage::{
    Attributes, Batch, Indexing, Log, PipelineMeters, Records, Resource, Severity, SignalIndexing,
    Storage, TraceContext, now_unix_nanos,
};
use otelo_indexed_storage_sqlite::{FrameMapper, Sqlite};
use otelo_journal::Journal;
use otelo_query::Signal;
use tokio::sync::watch;

const HOUR_NS: i64 = 3_600_000_000_000;
const FRAME_COUNT: u32 = 100;
const FIRST_FRAME_OF_SECOND_TRANSACTION: u32 = 64;

fn log_batch() -> Batch {
    vec![Records {
        resource: Resource {
            service: "shop".into(),
            attributes: Attributes::new(),
        },
        logs: vec![Log {
            logged_at: now_unix_nanos(),
            severity_number: Severity::INFO,
            body: "cart is empty".into(),
            trace_context: TraceContext::None,
            attributes: Attributes::new(),
        }],
        spans: Vec::new(),
        metrics: Vec::new(),
    }]
}

fn wait_for_indexing(
    indexing: &watch::Receiver<Indexing>,
    is_awaited: impl Fn(&Indexing) -> bool,
) -> Indexing {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = *indexing.borrow();
        if is_awaited(&current) {
            return current;
        }
        assert!(
            Instant::now() < deadline,
            "the indexing never came, the last was {current:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_indexer_reports_how_far_it_has_read_until_it_catches_up() {
    let data_directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(
        data_directory.path(),
        common::INDEX_RETENTION_DAYS,
        BTreeSet::new(),
    )
    .unwrap();
    let opened = common::open_journal(data_directory.path());
    let hour_ago = now_unix_nanos() - HOUR_NS;
    let frames: Vec<(u32, i64)> = (0..FRAME_COUNT)
        .map(|number| (number, hour_ago + i64::from(number) * HOUR_NS / 100))
        .collect();
    common::append_numbered_frames(opened.journal.as_ref(), Signal::Logs, &frames);
    let indexing = storage.watch_indexing();
    assert_eq!(*indexing.borrow(), Indexing::STARTING);

    let (release_sender, release_receiver) = mpsc::channel::<()>();
    let release_receiver = Mutex::new(release_receiver);
    let map_frame: FrameMapper = Arc::new(move |_, request| {
        let number = u32::from_le_bytes(request.try_into()?);
        if number == FIRST_FRAME_OF_SECOND_TRANSACTION {
            release_receiver.lock().unwrap().recv()?;
        }
        Ok(log_batch())
    });
    let indexer = storage
        .spawn_indexer(
            opened.journal as Arc<dyn Journal>,
            opened.synced_ends,
            map_frame,
            Arc::new(PipelineMeters::default()),
        )
        .unwrap();

    let indexing_after_first_transaction = wait_for_indexing(&indexing, |indexing| {
        indexing.logs != SignalIndexing::Starting
    });
    assert!(
        matches!(
            indexing_after_first_transaction.logs,
            SignalIndexing::CatchingUp {
                indexed_percent: 62 | 63
            }
        ),
        "{indexing_after_first_transaction:?}"
    );
    release_sender.send(()).unwrap();
    let indexing_after_catch_up = wait_for_indexing(&indexing, |indexing| {
        indexing.logs == SignalIndexing::CaughtUp
    });
    assert_eq!(
        indexing_after_catch_up,
        Indexing {
            logs: SignalIndexing::CaughtUp,
            spans: SignalIndexing::CaughtUp,
            metrics: SignalIndexing::CaughtUp,
        }
    );

    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();
}
