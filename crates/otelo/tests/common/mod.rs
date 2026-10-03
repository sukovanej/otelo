#![allow(dead_code, reason = "each test file uses a part")]

use std::fmt::{self, Write as _};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStderr, Command, Output, Stdio};
use std::time::{Duration, Instant};

pub struct Daemon {
    child: Child,
    stderr: BufReader<ChildStderr>,
    pub api_addr: String,
    pub otlp_http_addr: String,
    pub otlp_grpc_addr: String,
    pub password: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // A daemon left by a failed test would hold open the pipe of the test's output.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn start_daemon(data_dir: &Path) -> Daemon {
    // So a test finds only the telemetry it wrote.
    start_daemon_with_args(data_dir, &["--own-telemetry", "off"])
}

pub fn init_data_dir(data_dir: &Path, init_args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_otelo"))
        .arg("init")
        .arg("--data")
        .arg(data_dir)
        .args(init_args)
        .output()
        .unwrap()
}

pub fn make_password(data_dir: &Path) -> String {
    let output = init_data_dir(data_dir, &["--new-password"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

pub fn start_daemon_with_args(data_dir: &Path, args: &[&str]) -> Daemon {
    let password = make_password(data_dir);
    let mut child = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--otlp-http",
            "127.0.0.1:0",
        ])
        .args(["--otlp-grpc", "127.0.0.1:0", "--data"])
        .arg(data_dir)
        .args(args)
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
    let read_log_field = |name: &str| {
        line.split_whitespace()
            .find_map(|field| field.strip_prefix(name)?.strip_prefix('='))
            .unwrap()
            .to_owned()
    };
    Daemon {
        api_addr: read_log_field("addr"),
        otlp_http_addr: read_log_field("otlp_http"),
        otlp_grpc_addr: read_log_field("otlp_grpc"),
        child,
        stderr,
        password,
    }
}

impl Daemon {
    pub fn otelo_command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_otelo"));
        command
            .env("OTELO_URL", format!("http://{}", self.api_addr))
            .env("OTELO_PASSWORD", &self.password);
        command
    }

    pub fn bearer_header(&self) -> String {
        format!("Bearer {}", self.password)
    }
}

pub fn send_request(addr: &str, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
    send_request_with_body(addr, method, path, headers, "")
}

pub fn send_request_with_body(
    addr: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> String {
    let mut head = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !body.is_empty() {
        let _ = write!(head, "Content-Length: {}\r\n", body.len());
    }
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        let _ = write!(head, "Host: {addr}\r\n");
    }
    for (name, value) in headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    head.push_str(body);
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

pub fn send_get_request(daemon: &Daemon, path: &str) -> String {
    send_request(
        &daemon.api_addr,
        "GET",
        path,
        &[("Authorization", &daemon.bearer_header())],
    )
}

pub fn find_header_value<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    let (head, _) = response.split_once("\r\n\r\n")?;
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[derive(Clone, Copy)]
pub enum StopSignal {
    Term,
    Int,
}

impl fmt::Display for StopSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Term => "TERM",
            Self::Int => "INT",
        })
    }
}

pub fn stop_daemon(mut daemon: Daemon, signal: StopSignal) -> String {
    let pid = daemon.child.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-s", &signal.to_string(), &pid])
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
    let mut remaining_stderr = String::new();
    daemon.stderr.read_to_string(&mut remaining_stderr).unwrap();
    remaining_stderr
}
