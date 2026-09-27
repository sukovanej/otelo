#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::time::{Duration, Instant};

struct Daemon {
    child: Child,
    stderr: BufReader<ChildStderr>,
    addr: String,
}

/// Starts `siner serve` on a free port and waits for it to listen.
fn start(data: &std::path::Path) -> Daemon {
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

fn get(addr: &str, path: &str) -> String {
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
fn stop(mut daemon: Daemon, signal: &str) -> String {
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

#[test]
fn health_answers_200() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    let response = get(&daemon.addr, "/health");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    stop(daemon, "TERM");
}

#[test]
fn makes_the_missing_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("var/siner");
    let daemon = start(&data);
    assert!(data.is_dir());
    stop(daemon, "TERM");
}

#[test]
fn writes_telemetry_under_the_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    stop(start(dir.path()), "TERM");
    let files: Vec<_> = std::fs::read_dir(dir.path().join("telemetry"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        files.iter().any(|name| name.ends_with(".sqlite")),
        "{files:?}"
    );
}

#[test]
fn sigterm_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = stop(start(dir.path()), "TERM");
    assert!(log.contains("stopped"), "{log}");
}

#[test]
fn ctrl_c_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let log = stop(start(dir.path()), "INT");
    assert!(log.contains("stopped"), "{log}");
}
