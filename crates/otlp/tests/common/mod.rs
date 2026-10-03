#![allow(dead_code, reason = "each test file uses a part")]

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use otelo_indexed_storage::{BatchInbox, BatchSender, open_batch_channel};
use otelo_indexed_storage_sqlite::{Config, TELEMETRY_FILE_NAME, Writer};
use otelo_journal::Journal;
use otelo_journal_files::{JournalFiles, JournalThreads};
use otelo_otlp::Intake;
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
    sender: BatchSender,
    writer: Option<Writer>,
    pub journal: Arc<dyn Journal>,
    journal_threads: Option<JournalThreads>,
    _journal_directory: Option<TempDir>,
}

pub struct TestJournal {
    journal: Arc<dyn Journal>,
    threads: Option<JournalThreads>,
    directory: Option<TempDir>,
}

impl TestJournal {
    pub fn open_in(directory: &Path) -> Self {
        let (journal, threads) =
            JournalFiles::open(otelo_journal_files::Config::new(directory.join("journal")))
                .unwrap();
        Self {
            journal,
            threads: Some(threads),
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
            directory: None,
        }
    }
}

impl Receiver {
    pub fn start_writing_into(directory: &Path) -> Self {
        let (sender, inbox) = open_batch_channel(64);
        let writer = Writer::spawn(Config::new(directory.to_owned()), inbox).unwrap();
        Self::start_servers(sender, Some(writer), TestJournal::open_in(directory))
    }

    // Nothing reads the channel of one batch, so the second batch finds it full.
    pub fn start_with_full_queue() -> (Self, BatchInbox) {
        let (sender, inbox) = open_batch_channel(1);
        let journal = TestJournal::open_in_temporary_directory();
        (Self::start_servers(sender, None, journal), inbox)
    }

    pub fn start_with_journal(journal: TestJournal) -> (Self, BatchInbox) {
        let (sender, inbox) = open_batch_channel(64);
        (Self::start_servers(sender, None, journal), inbox)
    }

    fn start_servers(sender: BatchSender, writer: Option<Writer>, journal: TestJournal) -> Self {
        let intake = Intake::new(Arc::clone(&journal.journal), sender.clone());
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
            sender,
            writer,
            journal: journal.journal,
            journal_threads: journal.threads,
            _journal_directory: journal.directory,
        }
    }

    pub fn http_url(&self, path: &str) -> String {
        format!("http://{}{path}", self.http_address)
    }

    pub fn stop_and_wait_for_writer(self) {
        self.shutdown.cancel();
        for server in self.servers {
            self.runtime.block_on(server).unwrap().unwrap();
        }
        drop(self.sender);
        if let Some(writer) = self.writer {
            writer.join().unwrap();
        }
        if let Some(journal_threads) = self.journal_threads {
            journal_threads.stop_and_join().unwrap();
        }
    }
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
