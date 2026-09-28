//! Starts and stops `siner serve` for the tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Daemon {
    child: Child,
    stderr: BufReader<ChildStderr>,
    pub addr: String,
}

/// Starts `siner serve` on a free port and waits for it to listen.
pub fn start(data: &std::path::Path) -> Daemon {
    let mut child = Command::new(env!("CARGO_BIN_EXE_siner"))
        .args(["serve", "--listen", "127.0.0.1:0", "--data"])
        .arg(data)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(
            stderr.read_line(&mut line).unwrap(),
            0,
            "exited before it listened"
        );
        if line.contains("listening") {
            break;
        }
    }
    let addr = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("addr="))
        .unwrap()
        .to_owned();
    Daemon {
        child,
        stderr,
        addr,
    }
}

pub fn get(addr: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// Sends `signal` and returns the rest of stderr once the daemon exits.
pub fn stop(mut daemon: Daemon, signal: &str) -> String {
    let pid = daemon.child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-s", signal, &pid])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = daemon.child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            daemon.child.kill().unwrap();
            panic!("still running 10 s after SIG{signal}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "exited with {status}");
    let mut rest = String::new();
    daemon.stderr.read_to_string(&mut rest).unwrap();
    rest
}
