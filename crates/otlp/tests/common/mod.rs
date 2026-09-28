//! Runs the OTLP receiver in the test process, on a writer of its own.

#![allow(dead_code, reason = "each test file uses a part")]

use std::net::SocketAddr;
use std::path::Path;

use rusqlite::Connection;
use siner_telemetry::{Config, Day, Inbox, Sender, Writer, channel};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct Receiver {
    pub runtime: Runtime,
    pub http: SocketAddr,
    pub grpc: SocketAddr,
    shutdown: CancellationToken,
    servers: Vec<JoinHandle<anyhow::Result<()>>>,
    sender: Sender,
    writer: Option<Writer>,
}

impl Receiver {
    /// Serves both transports on free ports and writes into `dir`.
    pub fn start(dir: &Path) -> Self {
        let (sender, inbox) = channel(64);
        let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
        Self::serve(sender, Some(writer))
    }

    /// Serves both transports with a channel of one batch that nothing reads,
    /// so the second batch finds it full.
    pub fn full() -> (Self, Inbox) {
        let (sender, inbox) = channel(1);
        (Self::serve(sender, None), inbox)
    }

    fn serve(sender: Sender, writer: Option<Writer>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let shutdown = CancellationToken::new();
        let (http, grpc, servers) = runtime.block_on(async {
            let http = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let grpc = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addrs = (http.local_addr().unwrap(), grpc.local_addr().unwrap());
            let servers = vec![
                tokio::spawn(siner_otlp::serve_http(
                    http,
                    sender.clone(),
                    shutdown.clone(),
                )),
                tokio::spawn(siner_otlp::serve_grpc(
                    grpc,
                    sender.clone(),
                    shutdown.clone(),
                )),
            ];
            (addrs.0, addrs.1, servers)
        });
        Self {
            runtime,
            http,
            grpc,
            shutdown,
            servers,
            sender,
            writer,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.http)
    }

    /// Stops the servers and waits until the writer wrote what they took.
    pub fn stop(self) {
        self.shutdown.cancel();
        for server in self.servers {
            self.runtime.block_on(server).unwrap().unwrap();
        }
        drop(self.sender);
        if let Some(writer) = self.writer {
            writer.join().unwrap();
        }
    }
}

/// Today's day file in `dir`.
pub fn today(dir: &Path) -> Connection {
    Connection::open(dir.join(Day::today().file_name())).unwrap()
}

pub fn rows<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str) -> Vec<T> {
    conn.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
