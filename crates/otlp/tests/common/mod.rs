#![allow(dead_code, reason = "each test file uses a part")]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use otelo_indexed_storage::query::{LogLine, PageRequest, SpanSort, TraceSpan};
use otelo_indexed_storage::{PipelineMeters, RangeQueries, TimeRange, now_unix_nanos};
use otelo_indexed_storage_sqlite::{Config, FrameMapper, Indexer, Reader, TELEMETRY_FILE_NAME};
use otelo_journal::{Journal, SyncedEndInbox};
use otelo_journal_files::{JournalFiles, JournalThreads};
use otelo_otlp::Intake;
use otelo_query::{Query, Signal};
use rusqlite::Connection;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct Receiver {
    pub runtime: Runtime,
    pub http_address: SocketAddr,
    pub grpc_address: SocketAddr,
    shutdown: CancellationToken,
    servers: Vec<JoinHandle<anyhow::Result<()>>>,
    indexer: Option<Indexer>,
    pub meters: Arc<PipelineMeters>,
    pub journal: Arc<dyn Journal>,
    journal_threads: Option<JournalThreads>,
    _journal_directory: Option<TempDir>,
}

pub struct TestJournal {
    journal: Arc<dyn Journal>,
    threads: Option<JournalThreads>,
    synced_ends: Option<SyncedEndInbox>,
    directory: Option<TempDir>,
}

impl TestJournal {
    pub fn open_in(directory: &Path) -> Self {
        let opened =
            JournalFiles::open(otelo_journal_files::Config::new(directory.join("journal")))
                .unwrap();
        Self {
            journal: opened.journal,
            threads: Some(opened.threads),
            synced_ends: Some(opened.synced_ends),
            directory: None,
        }
    }

    pub fn open_in_temporary_directory() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let journal = Self::open_in(directory.path());
        Self {
            directory: Some(directory),
            ..journal
        }
    }

    pub fn of(journal: Arc<dyn Journal>) -> Self {
        Self {
            journal,
            threads: None,
            synced_ends: None,
            directory: None,
        }
    }
}

impl Receiver {
    pub fn start_writing_into(directory: &Path) -> Self {
        let mut journal = TestJournal::open_in(directory);
        let meters = Arc::new(PipelineMeters::default());
        let map_frame: FrameMapper = Arc::new(otelo_otlp::map::map_journal_frame);
        let indexer = Indexer::spawn(
            Config::new(directory.to_owned()),
            Arc::clone(&journal.journal),
            journal.synced_ends.take().unwrap(),
            map_frame,
            Arc::clone(&meters),
        )
        .unwrap();
        Self::start_servers(Some(indexer), meters, journal)
    }

    pub fn start_with_journal(journal: TestJournal) -> Self {
        Self::start_servers(None, Arc::new(PipelineMeters::default()), journal)
    }

    fn start_servers(
        indexer: Option<Indexer>,
        meters: Arc<PipelineMeters>,
        journal: TestJournal,
    ) -> Self {
        let intake = Intake::new(Arc::clone(&journal.journal), Arc::clone(&meters));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let shutdown = CancellationToken::new();
        let (http_address, grpc_address, servers) = runtime.block_on(async {
            let http_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let grpc_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let http_address = http_listener.local_addr().unwrap();
            let grpc_address = grpc_listener.local_addr().unwrap();
            let servers = vec![
                tokio::spawn(otelo_otlp::serve_http(
                    http_listener,
                    intake.clone(),
                    shutdown.clone(),
                )),
                tokio::spawn(otelo_otlp::serve_grpc(
                    grpc_listener,
                    intake,
                    shutdown.clone(),
                )),
            ];
            (http_address, grpc_address, servers)
        });
        Self {
            runtime,
            http_address,
            grpc_address,
            shutdown,
            servers,
            indexer,
            meters,
            journal: journal.journal,
            journal_threads: journal.threads,
            _journal_directory: journal.directory,
        }
    }

    pub fn http_url(&self, path: &str) -> String {
        format!("http://{}{path}", self.http_address)
    }

    pub fn stop_and_wait_for_indexer(self) {
        self.shutdown.cancel();
        for server in self.servers {
            self.runtime.block_on(server).unwrap().unwrap();
        }
        if let Some(journal_threads) = self.journal_threads {
            journal_threads.stop_and_join().unwrap();
        }
        if let Some(indexer) = self.indexer {
            indexer.join().unwrap();
        }
    }
}

const DAY: i64 = 86_400 * 1_000_000_000;

fn open_reader_around_now(directory: &Path) -> Reader {
    let now = now_unix_nanos();
    Reader::open(directory, TimeRange::new(now - DAY, now + DAY).unwrap()).unwrap()
}

pub fn read_spans_oldest_first(directory: &Path) -> Vec<TraceSpan> {
    open_reader_around_now(directory)
        .list_spans(
            &Query::all(Signal::Spans),
            SpanSort::Oldest,
            &PageRequest::first(1000),
        )
        .unwrap()
        .spans
}

pub fn read_logs_oldest_first(directory: &Path) -> Vec<LogLine> {
    let mut logs = open_reader_around_now(directory)
        .list_logs(&Query::all(Signal::Logs), &PageRequest::first(1000))
        .unwrap()
        .logs;
    logs.reverse();
    logs
}

pub fn open_telemetry_file(directory: &Path) -> Connection {
    Connection::open(directory.join(TELEMETRY_FILE_NAME)).unwrap()
}

pub fn query_first_column<T: rusqlite::types::FromSql>(
    connection: &Connection,
    sql: &str,
) -> Vec<T> {
    connection
        .prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
