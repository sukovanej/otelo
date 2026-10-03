#![cfg(unix)]

mod common;

use std::path::{Path, PathBuf};

use common::{
    StopSignal, find_header_value, send_get_request, send_request, start_daemon, stop_daemon,
};

fn ui_dist_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/app/dist")
}

fn ui_is_built() -> bool {
    ui_dist_dir().join("index.html").exists()
}

#[test]
fn every_page_gets_the_index_of_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    for path in [
        "/",
        "/logs",
        "/logs?q=level%20%3E%3D%20warn&view=groups",
        "/traces?view=spans",
        "/traces/0af7651916cd43dd8448eb211c80319c?span=b7ad6b7169203331",
    ] {
        let response = send_get_request(&daemon, path);
        if ui_is_built() {
            assert!(response.starts_with("HTTP/1.1 200"), "{path}: {response}");
            assert_eq!(
                find_header_value(&response, "content-type"),
                Some("text/html")
            );
            assert_eq!(
                find_header_value(&response, "cache-control"),
                Some("no-cache")
            );
            assert!(response.contains("id=\"root\""), "{path}: {response}");
        } else {
            assert!(response.starts_with("HTTP/1.1 503"), "{path}: {response}");
            assert!(response.contains("mise run web:build"), "{response}");
        }
    }
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn an_unchanged_file_is_not_sent_again() {
    if !ui_is_built() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let first_response = send_get_request(&daemon, "/");
    let etag = find_header_value(&first_response, "etag").unwrap();
    let second_response = send_request(&daemon.api_addr, "GET", "/", &[("If-None-Match", etag)]);
    assert!(
        second_response.starts_with("HTTP/1.1 304"),
        "{second_response}"
    );
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_api_and_the_assets_do_not_fall_back_to_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    let api_response = send_get_request(&daemon, "/api/nothing");
    assert!(api_response.starts_with("HTTP/1.1 404"), "{api_response}");
    assert!(
        api_response.contains(r#"{"error":"no endpoint at /api/nothing"}"#),
        "{api_response}"
    );
    let asset_response = send_get_request(&daemon, "/assets/index-gone.js");
    assert!(
        asset_response.starts_with("HTTP/1.1 404"),
        "{asset_response}"
    );
    let post_response = send_request(
        &daemon.api_addr,
        "POST",
        "/logs",
        &[("Content-Length", "0")],
    );
    assert!(post_response.starts_with("HTTP/1.1 405"), "{post_response}");
    stop_daemon(daemon, StopSignal::Term);
}

#[test]
fn the_fonts_are_served_as_woff2_for_good() {
    if !ui_is_built() {
        return;
    }
    let font = std::fs::read_dir(ui_dist_dir().join("assets"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| {
            name.starts_with("ibm-plex-sans-latin-400")
                && Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension == "woff2")
        })
        .expect("the build has IBM Plex Sans");
    let dir = tempfile::tempdir().unwrap();
    let daemon = start_daemon(dir.path());
    // HEAD, since the body is not text.
    let response = send_request(&daemon.api_addr, "HEAD", &format!("/assets/{font}"), &[]);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert_eq!(
        find_header_value(&response, "content-type"),
        Some("font/woff2")
    );
    assert_eq!(
        find_header_value(&response, "cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    stop_daemon(daemon, StopSignal::Term);
}
