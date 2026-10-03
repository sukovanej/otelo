#![cfg(unix)]

mod common;

use std::process::{Command, Stdio};

use common::{
    Daemon, StopSignal, find_header_value, init_data_dir, send_get_request, send_request,
    send_request_with_body, start_daemon, start_daemon_without_auth, stop_daemon,
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

fn get_services_with_cookie(daemon: &Daemon, password: &str) -> String {
    send_request(
        &daemon.api_addr,
        "GET",
        "/api/services",
        &[("Cookie", &format!("theme=dark; otelo_password={password}"))],
    )
}

fn get_services_with_bearer(daemon: &Daemon, password: &str) -> String {
    send_request(
        &daemon.api_addr,
        "GET",
        "/api/services",
        &[("Authorization", &format!("Bearer {password}"))],
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
    assert!(!String::from_utf8_lossy(&state).contains(password));

    let second = init_data_dir(dir.path(), &[]);
    assert!(!second.status.success());
    let stderr = String::from_utf8(second.stderr).unwrap();
    assert!(stderr.contains("--new-password"), "{stderr}");
}

#[test]
fn a_new_password_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let output = init_data_dir(dir.path(), &["--new-password"]);
    assert!(output.status.success());
    let new_password = String::from_utf8(output.stdout).unwrap().trim().to_owned();
    assert_ne!(new_password, daemon.password);
    let response = send_get_request(&daemon, "/api/services");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    let response = get_services_with_bearer(&daemon, &new_password);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
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
fn unsafe_no_auth_serves_the_api_without_a_password() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon_without_auth(dir.path());
    let response = send_request(&daemon.api_addr, "GET", "/api/services", &[]);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let output = daemon
        .otelo_command()
        .env_remove("OTELO_PASSWORD")
        .args(["services", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn unsafe_no_auth_does_not_listen_beyond_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args([
            "serve",
            "--unsafe-no-auth",
            "--listen",
            "0.0.0.0:0",
            "--data",
        ])
        .arg(dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("loopback"), "{stderr}");
}

#[test]
fn the_api_needs_the_password_and_the_ui_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    for path in ["/api/services", "/api/openapi.json", "/api/nothing"] {
        let response = send_request(&daemon.api_addr, "GET", path, &[]);
        assert!(response.starts_with("HTTP/1.1 401"), "{path}: {response}");
    }
    let response = get_services_with_bearer(&daemon, "WRONGPASSWORD");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    let response = get_services_with_cookie(&daemon, "WRONGPASSWORD");
    assert!(response.starts_with("HTTP/1.1 401"), "{response}");
    let response = get_services_with_bearer(&daemon, &daemon.password);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let response = get_services_with_cookie(&daemon, &daemon.password);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    for path in ["/health", "/", "/logs"] {
        let response = send_request(&daemon.api_addr, "GET", path, &[]);
        assert!(!response.starts_with("HTTP/1.1 401"), "{path}: {response}");
    }
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_login_sets_the_password_as_a_cookie_and_the_logout_clears_it() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let wrong = send_login(&daemon, "WRONGPASSWORD", &[]);
    assert!(wrong.starts_with("HTTP/1.1 401"), "{wrong}");
    assert!(find_header_value(&wrong, "set-cookie").is_none());

    let right = send_login(&daemon, &daemon.password, &[]);
    assert!(right.starts_with("HTTP/1.1 204"), "{right}");
    let cookie = find_header_value(&right, "set-cookie").unwrap();
    assert!(
        cookie.starts_with(&format!("otelo_password={};", daemon.password)),
        "{cookie}"
    );
    for attribute in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(attribute), "{cookie}");
    }
    assert!(!cookie.contains("Secure"), "{cookie}");
    let behind_a_proxy = send_login(&daemon, &daemon.password, &[("Host", "otelo.example.com")]);
    let cookie = find_header_value(&behind_a_proxy, "set-cookie").unwrap();
    assert!(cookie.contains("; Secure"), "{cookie}");

    let logout = send_request(&daemon.api_addr, "POST", "/api/logout", &[]);
    assert!(logout.starts_with("HTTP/1.1 204"), "{logout}");
    let cookie = find_header_value(&logout, "set-cookie").unwrap();
    assert!(cookie.starts_with("otelo_password=;"), "{cookie}");
    assert!(cookie.contains("Max-Age=0"), "{cookie}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_cli_sends_otelo_password() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let without_password = daemon
        .otelo_command()
        .env_remove("OTELO_PASSWORD")
        .args(["services", "--json"])
        .output()
        .unwrap();
    assert!(!without_password.status.success());
    let stderr = String::from_utf8(without_password.stderr).unwrap();
    assert!(stderr.contains("set OTELO_PASSWORD"), "{stderr}");
    let with_password = daemon
        .otelo_command()
        .args(["services", "--json"])
        .output()
        .unwrap();
    assert!(
        with_password.status.success(),
        "{}",
        String::from_utf8_lossy(&with_password.stderr)
    );
    stop_daemon(daemon, StopSignal::Term);
}
