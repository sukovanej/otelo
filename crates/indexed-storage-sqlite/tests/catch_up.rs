mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use otelo_indexed_storage::{
    Attributes, Batch, Log, PipelineMeters, Records, Resource, Severity, TraceContext,
    now_unix_nanos,
};
use otelo_indexed_storage_sqlite::Sqlite;
use otelo_journal::{Frames, Journal, Position, SyncTicket};
use otelo_query::Signal;
use tracing::field::{Field, Visit};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

#[derive(Clone, Default)]
struct LoggedMessages(Arc<Mutex<Vec<String>>>);

struct MessageVisitor<'a>(&'a mut Vec<String>);

impl Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push(format!("{value:?}"));
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for LoggedMessages {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        event.record(&mut MessageVisitor(&mut self.0.lock().unwrap()));
    }
}

struct JournalWithUnreadableSpans(Arc<dyn Journal>);

impl Journal for JournalWithUnreadableSpans {
    fn append_frame(
        &self,
        signal: Signal,
        received_at: i64,
        request: &[u8],
    ) -> anyhow::Result<SyncTicket> {
        self.0.append_frame(signal, received_at, request)
    }

    fn read_frames(&self, signal: Signal, from: Option<Position>) -> anyhow::Result<Frames> {
        match signal {
            Signal::Spans => Err(anyhow::anyhow!("the segment does not decompress")),
            Signal::Logs | Signal::Metrics => self.0.read_frames(signal, from),
        }
    }

    fn size_in_bytes(&self) -> anyhow::Result<u64> {
        self.0.size_in_bytes()
    }
}

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

fn count_catch_ups(
    logged_messages: &LoggedMessages,
    wrap_journal: impl FnOnce(Arc<dyn Journal>) -> Arc<dyn Journal>,
) -> usize {
    logged_messages.0.lock().unwrap().clear();
    let data_directory = tempfile::tempdir().unwrap();
    let storage = Sqlite::open(data_directory.path(), BTreeSet::new()).unwrap();
    let opened = common::open_journal(data_directory.path());
    common::append_numbered_frames(
        opened.journal.as_ref(),
        Signal::Logs,
        &[(0, now_unix_nanos())],
    );
    let indexer = storage
        .spawn_indexer(
            wrap_journal(opened.journal as Arc<dyn Journal>),
            opened.synced_ends,
            common::map_numbered_frames(vec![log_batch()]),
            Arc::new(PipelineMeters::default()),
        )
        .unwrap();
    opened.threads.stop_and_join().unwrap();
    indexer.join().unwrap();
    logged_messages
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|message| *message == "caught up with the journal")
        .count()
}

// The indexer logs on its own thread, so the subscriber is the global one of this test binary.
#[test]
fn the_indexer_logs_its_catch_up_only_once_every_signal_reads() {
    let logged_messages = LoggedMessages::default();
    tracing::subscriber::set_global_default(Registry::default().with(logged_messages.clone()))
        .unwrap();

    assert_eq!(count_catch_ups(&logged_messages, |journal| journal), 1);
    assert_eq!(
        count_catch_ups(&logged_messages, |journal| Arc::new(
            JournalWithUnreadableSpans(journal)
        )),
        0
    );
}
