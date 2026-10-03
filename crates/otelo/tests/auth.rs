#![cfg(unix)]

mod common;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{
    Daemon, StopSignal, find_header_value, init_data_dir, log_in_with_cli,
    read_saved_session_token, send_get_request, send_request, send_request_with_body, start_daemon,
    stop_daemon,
};

fn send_login(daemon: &Daemon, password: &str, headers: &[(&str, &str)]) -> String {
    let mut headers = headers.to_vec();
    headers.push(("Content-Type", "application/json"));
    send_request_with_body(
        &daemon.api_addr,
        "POST",
        "/api/login",
        &headers,
        &format!(r#"{{"password":"{password}"}}"#),
    )
}

fn read_session_cookie(response: &str) -> String {
    find_header_value(response, "set-cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("otelo_session=")
        .unwrap()
        .to_owned()
}

fn get_services_with_cookie(daemon: &Daemon, token: &str) -> String {
    send_request(
        &daemon.api_addr,
        "GET",
        "/api/services",
        &[("Cookie", &format!("theme=dark; otelo_session={token}"))],
    )
}

fn get_services_with_bearer(daemon: &Daemon, token: &str) -> String {
    send_request(
        &daemon.api_addr,
        "GET",
        "/api/services",
        &[("Authorization", &format!("Bearer {token}"))],
    )
}

#[test]
fn init_prints_a_password_and_keeps_only_its_hash() {
    let dir = tempfile::tempdir().unwrap();
    let output = init_data_dir(dir.path(), &[]);
    assert!(output.status.success());
    let password = String::from_utf8(output.stdout).unwrap();
    let password = password.trim();
    assert_eq!(password.len(), 26, "{password}");
    let state = std::fs::read(dir.path().join("state.sqlite")).unwrap();
    let state_text = String::from_utf8_lossy(&state);
    assert!(state_text.contains("$argon2id$"));
    assert!(!state_text.contains(password));

    let second = init_data_dir(dir.path(), &[]);
    assert!(!second.status.success());
    let stderr = String::from_utf8(second.stderr).unwrap();
    assert!(stderr.contains("--new-password"), "{stderr}");
}

#[test]
fn a_new_password_ends_every_session() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let old_password = daemon.password.clone();
    let output = init_data_dir(dir.path(), &["--new-password"]);
    assert!(output.status.success());
    let new_password = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert_ne!(new_password, old_password);
    let response = send_get_request(&daemon, "/api/services");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    let login = send_login(&daemon, &new_password, &[]);
    assert!(login.starts_with("HTTP/1.1 204"), "{login}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn serve_without_a_password_does_not_start() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--otlp-http",
            "127.0.0.1:0",
        ])
        .args([
            "--otlp-grpc",
            "127.0.0.1:0",
            "--own-telemetry",
            "off",
            "--data",
        ])
        .arg(dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("otelo init"), "{stderr}");
}

#[test]
fn the_right_password_starts_a_session_and_a_wrong_one_waits() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());

    let started_at = Instant::now();
    let wrong = send_login(&daemon, "WRONGPASSWORD", &[]);
    assert!(wrong.starts_with("HTTP/1.1 401"), "{wrong}");
    assert!(find_header_value(&wrong, "set-cookie").is_none());
    assert!(started_at.elapsed() >= Duration::from_secs(1));

    let right = send_login(&daemon, &daemon.password, &[]);
    assert!(right.starts_with("HTTP/1.1 204"), "{right}");
    let cookie = find_header_value(&right, "set-cookie").unwrap();
    for attribute in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(attribute), "{cookie}");
    }
    assert!(!cookie.contains("Secure"), "{cookie}");
    let token = read_session_cookie(&right);
    let response = get_services_with_cookie(&daemon, &token);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let response = get_services_with_bearer(&daemon, &token);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    let behind_a_proxy = send_login(&daemon, &daemon.password, &[("Host", "otelo.example.com")]);
    let cookie = find_header_value(&behind_a_proxy, "set-cookie").unwrap();
    assert!(cookie.contains("; Secure"), "{cookie}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_api_needs_a_live_session_and_the_ui_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    for path in ["/api/services", "/api/openapi.json", "/api/nothing"] {
        let response = send_request(&daemon.api_addr, "GET", path, &[]);
        assert!(response.starts_with("HTTP/1.1 401"), "{path}: {response}");
    }
    let response = get_services_with_bearer(&daemon, "not-a-token");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    for path in ["/health", "/", "/logs"] {
        let response = send_request(&daemon.api_addr, "GET", path, &[]);
        assert!(!response.starts_with("HTTP/1.1 401"), "{path}: {response}");
    }

    let expiring = read_session_cookie(&send_login(&daemon, &daemon.password, &[]));
    let state = rusqlite::Connection::open(dir.path().join("state.sqlite")).unwrap();
    state
        .execute("UPDATE sessions SET last_used_at = 0", [])
        .unwrap();
    let response = get_services_with_bearer(&daemon, &expiring);
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");

    let ending = read_session_cookie(&send_login(&daemon, &daemon.password, &[]));
    let logout = send_request(
        &daemon.api_addr,
        "POST",
        "/api/logout",
        &[("Cookie", &format!("otelo_session={ending}"))],
    );
    assert!(logout.starts_with("HTTP/1.1 204"), "{logout}");
    assert!(
        find_header_value(&logout, "set-cookie").is_some_and(|cookie| cookie.contains("Max-Age=0")),
        "{logout}"
    );
    let response = get_services_with_cookie(&daemon, &ending);
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn a_write_from_another_origin_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let bearer = daemon.bearer_header();
    let own_origin = format!("http://{}", daemon.api_addr);
    for (origin, status) in [
        ("https://evil.example.com", "403"),
        ("null", "403"),
        (own_origin.as_str(), "200"),
    ] {
        let response = send_request(
            &daemon.api_addr,
            "PUT",
            "/api/indexes/logs/user.id",
            &[("Authorization", &bearer), ("Origin", origin)],
        );
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "{origin}: {response}"
        );
    }
    let login = send_login(
        &daemon,
        &daemon.password,
        &[("Origin", "https://evil.example.com")],
    );
    assert!(login.starts_with("HTTP/1.1 403"), "{login}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn otelo_login_keeps_the_session_for_the_next_commands() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let config_dir = tempfile::tempdir().unwrap();
    let run_services = || {
        Command::new(env!("CARGO_BIN_EXE_otelo"))
            .args(["services", "--json"])
            .env("OTELO_URL", format!("http://{}", daemon.api_addr))
            .env("XDG_CONFIG_HOME", config_dir.path())
            .output()
            .unwrap()
    };

    let before_login = run_services();
    assert!(!before_login.status.success());
    let stderr = String::from_utf8(before_login.stderr).unwrap();
    assert!(stderr.contains("otelo login"), "{stderr}");

    let wrong_login = log_in_with_cli(&daemon.api_addr, config_dir.path(), "WRONGPASSWORD");
    assert!(!wrong_login.status.success());
    assert!(read_saved_session_token(config_dir.path(), &daemon.api_addr).is_none());

    let login = log_in_with_cli(&daemon.api_addr, config_dir.path(), &daemon.password);
    assert!(
        login.status.success(),
        "{}",
        String::from_utf8_lossy(&login.stderr)
    );
    let sessions_path = config_dir.path().join("otelo/sessions.json");
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(&sessions_path).unwrap().permissions(),
    );
    assert_eq!(mode & 0o777, 0o600);
    let after_login = run_services();
    assert!(
        after_login.status.success(),
        "{}",
        String::from_utf8_lossy(&after_login.stderr)
    );

    let logout = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .arg("logout")
        .env("OTELO_URL", format!("http://{}", daemon.api_addr))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .output()
        .unwrap();
    assert!(logout.status.success());
    assert!(read_saved_session_token(config_dir.path(), &daemon.api_addr).is_none());
    assert!(!run_services().status.success());
    stop_daemon(daemon, StopSignal::Term);
}
