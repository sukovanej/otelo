use std::path::Path;
use std::process::{Command, Output};

fn run_otelo(config_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args(args)
        .env("XDG_CONFIG_HOME", config_dir)
        .env_remove("OTELO_URL")
        .output()
        .unwrap()
}

fn read_stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn add_writes_the_address_of_a_remote_to_remote_json() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(
        config_dir.path(),
        &["remote", "add", "mudro", "https://otelo.mudro.cz/"],
    );
    assert!(output.status.success(), "{}", read_stderr(&output));

    let remotes_json =
        std::fs::read_to_string(config_dir.path().join("otelo/remote.json")).unwrap();
    let remotes: serde_json::Value = serde_json::from_str(&remotes_json).unwrap();
    assert_eq!(
        remotes,
        serde_json::json!({"mudro": {"address": "https://otelo.mudro.cz"}})
    );

    let output = run_otelo(config_dir.path(), &["remote", "list", "--json"]);
    assert!(output.status.success(), "{}", read_stderr(&output));
    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed, remotes);
}

#[test]
fn add_keeps_a_remote_of_the_same_name() {
    let config_dir = tempfile::tempdir().unwrap();
    run_otelo(
        config_dir.path(),
        &["remote", "add", "mudro", "https://a.example"],
    );
    let output = run_otelo(
        config_dir.path(),
        &["remote", "add", "mudro", "https://b.example"],
    );
    assert!(!output.status.success());
    assert!(read_stderr(&output).contains("otelo remote remove mudro"));
    let output = run_otelo(config_dir.path(), &["remote", "list", "--json"]);
    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed["mudro"]["address"], "https://a.example");
}

#[test]
fn add_refuses_an_address_without_http() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(
        config_dir.path(),
        &["remote", "add", "mudro", "otelo.mudro.cz"],
    );
    assert!(!output.status.success());
    assert!(read_stderr(&output).contains("http://"));
    assert!(!config_dir.path().join("otelo/remote.json").exists());
}

#[test]
fn list_without_remote_json_prints_no_remotes() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(config_dir.path(), &["remote", "list", "--json"]);
    assert!(output.status.success(), "{}", read_stderr(&output));
    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(listed, serde_json::json!({}));
}

#[test]
fn add_refuses_a_name_that_daemon_reads_as_an_address() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(
        config_dir.path(),
        &[
            "remote",
            "add",
            "otelo.mudro.cz:443",
            "https://otelo.mudro.cz",
        ],
    );
    assert!(!output.status.success());
    assert!(read_stderr(&output).contains("as an address"));
}

#[test]
fn daemon_of_a_name_queries_the_remote_of_that_name() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(config_dir.path(), &["services", "--daemon", "mudro"]);
    assert!(!output.status.success());
    assert!(
        read_stderr(&output).contains("otelo remote add mudro"),
        "{}",
        read_stderr(&output)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_otelo"))
        .args(["services"])
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("OTELO_URL", "mudro")
        .output()
        .unwrap();
    assert!(read_stderr(&output).contains("no remote is named mudro"));
}

#[test]
fn daemon_of_an_address_without_http_is_refused() {
    let config_dir = tempfile::tempdir().unwrap();
    let output = run_otelo(
        config_dir.path(),
        &["services", "--daemon", "127.0.0.1:7070"],
    );
    assert!(!output.status.success());
    assert!(read_stderr(&output).contains("http://"));
}
