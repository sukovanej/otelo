#![cfg(unix)]

mod common;

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use common::{get, start, stop};

/// Where `mise run web:build` puts the UI. A debug build reads it from here.
fn dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/app/dist")
}

/// Whether `mise run web:build` has run.
fn built() -> bool {
    dist().join("index.html").exists()
}

/// Sends a request without a body and returns the whole response.
fn request(addr: &str, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (name, value) in headers {
        let _ = write!(head, "{name}: {value}\r\n");
    }
    head.push_str("\r\n");
    let mut stream = TcpStream::connect(addr).unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

/// The value of a header of a response.
fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    let (head, _) = response.split_once("\r\n\r\n")?;
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

#[test]
fn every_page_gets_the_index_of_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    for path in [
        "/",
        "/logs",
        "/logs?q=level%20%3E%3D%20warn&view=groups",
        "/traces?view=spans",
        "/traces/0af7651916cd43dd8448eb211c80319c?span=b7ad6b7169203331",
    ] {
        let response = get(&daemon.addr, path);
        if built() {
            assert!(response.starts_with("HTTP/1.1 200"), "{path}: {response}");
            assert_eq!(header(&response, "content-type"), Some("text/html"));
            assert_eq!(header(&response, "cache-control"), Some("no-cache"));
            assert!(response.contains("id=\"root\""), "{path}: {response}");
        } else {
            assert!(response.starts_with("HTTP/1.1 503"), "{path}: {response}");
            assert!(response.contains("mise run web:build"), "{response}");
        }
    }
    stop(daemon, "TERM");
}

#[test]
fn an_unchanged_file_is_not_sent_again() {
    if !built() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    let first = get(&daemon.addr, "/");
    let etag = header(&first, "etag").unwrap();
    let again = request(&daemon.addr, "GET", "/", &[("If-None-Match", etag)]);
    assert!(again.starts_with("HTTP/1.1 304"), "{again}");
    stop(daemon, "TERM");
}

#[test]
fn the_api_and_the_assets_do_not_fall_back_to_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let daemon = start(dir.path());
    let api = get(&daemon.addr, "/api/nothing");
    assert!(api.starts_with("HTTP/1.1 404"), "{api}");
    assert!(
        api.contains(r#"{"error":"no endpoint at /api/nothing"}"#),
        "{api}"
    );
    let asset = get(&daemon.addr, "/assets/index-gone.js");
    assert!(asset.starts_with("HTTP/1.1 404"), "{asset}");
    let post = request(&daemon.addr, "POST", "/logs", &[("Content-Length", "0")]);
    assert!(post.starts_with("HTTP/1.1 405"), "{post}");
    stop(daemon, "TERM");
}

#[test]
fn the_fonts_are_served_as_woff2_for_good() {
    if !built() {
        return;
    }
    let font = std::fs::read_dir(dist().join("assets"))
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
    let daemon = start(dir.path());
    // HEAD, since the body is not text.
    let response = request(&daemon.addr, "HEAD", &format!("/assets/{font}"), &[]);
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert_eq!(header(&response, "content-type"), Some("font/woff2"));
    assert_eq!(
        header(&response, "cache-control"),
        Some("public, max-age=31536000, immutable")
    );
    stop(daemon, "TERM");
}
