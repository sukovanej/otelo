#![allow(dead_code, reason = "each test file uses a part")]

use std::net::SocketAddr;
use std::path::Path;

use otelo_storage::{Inbox, Sender, batch_channel};
use otelo_storage_sqlite::{Config, Day, Writer};
use rusqlite::Connection;
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
    pub fn start_writing_into(dir: &Path) -> Self {
        let (sender, inbox) = batch_channel(64);
        let writer = Writer::spawn(Config::new(dir.to_owned()), inbox).unwrap();
        Self::serve(sender, Some(writer))
    }

    // Nothing reads the channel of one batch, so the second batch finds it full.
    pub fn start_with_full_queue() -> (Self, Inbox) {
        let (sender, inbox) = batch_channel(1);
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
                tokio::spawn(otelo_otlp::serve_http(
                    http,
                    sender.clone(),
                    shutdown.clone(),
                )),
                tokio::spawn(otelo_otlp::serve_grpc(
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

    pub fn stop_and_wait_for_writer(self) {
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

pub fn open_todays_day_file(dir: &Path) -> Connection {
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
