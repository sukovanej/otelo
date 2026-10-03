use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use data_encoding::HEXLOWER;
use otelo_update::{Channel, GithubReleases, UpdateStep};
use semver::Version;
use sha2::{Digest as _, Sha256};

struct FakeRelease {
    tag: &'static str,
    title: &'static str,
    assets: HashMap<String, Vec<u8>>,
}

struct FakeGithub {
    release: FakeRelease,
    base_url: String,
}

fn serve_fake_github(release: FakeRelease) -> (tokio::runtime::Runtime, String) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let github = Arc::new(FakeGithub {
        release,
        base_url: base_url.clone(),
    });
    let app = Router::new()
        .route("/repos/acme/otelo/releases/latest", get(serve_release_json))
        .route(
            "/repos/acme/otelo/releases/tags/canary",
            get(serve_release_json),
        )
        .route("/download/{name}", get(serve_asset))
        .with_state(github);
    runtime.spawn(async move { axum::serve(listener, app).await.unwrap() });
    (runtime, base_url)
}

async fn serve_release_json(State(github): State<Arc<FakeGithub>>) -> impl IntoResponse {
    let assets: Vec<serde_json::Value> = github
        .release
        .assets
        .keys()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "browser_download_url": format!("{}/download/{name}", github.base_url),
            })
        })
        .collect();
    axum::Json(serde_json::json!({
        "tag_name": github.release.tag,
        "name": github.release.title,
        "assets": assets,
    }))
}

async fn serve_asset(
    State(github): State<Arc<FakeGithub>>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    github
        .release
        .assets
        .get(&name)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)
}

fn write_digest_file(bytes: &[u8], name: &str) -> Vec<u8> {
    let digest = HEXLOWER.encode(&Sha256::digest(bytes));
    format!("{digest} *{name}\n").into_bytes()
}

fn release_with_archive(archive: &[u8], digest_file: &[u8]) -> FakeRelease {
    FakeRelease {
        tag: "v1.2.3",
        title: "v1.2.3",
        assets: HashMap::from([
            ("otelo-x.tar.gz".to_owned(), archive.to_vec()),
            ("otelo-x.tar.gz.sha256".to_owned(), digest_file.to_vec()),
        ]),
    }
}

fn release_titled(tag: &'static str, title: &'static str) -> FakeRelease {
    FakeRelease {
        tag,
        title,
        assets: HashMap::new(),
    }
}

fn parse_version(version: &str) -> Version {
    Version::parse(version).unwrap()
}

#[test]
fn the_latest_release_parses_its_tag_as_a_version() {
    let (_runtime, base_url) = serve_fake_github(release_with_archive(b"bytes", b""));

    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Stable)
        .unwrap();

    assert_eq!(release.version, Version::new(1, 2, 3));
    assert_eq!(release.tag, "v1.2.3");
}

#[test]
fn a_verified_download_returns_the_bytes() {
    let archive = b"the archive";
    let (_runtime, base_url) = serve_fake_github(release_with_archive(
        archive,
        &write_digest_file(archive, "otelo-x.tar.gz"),
    ));
    let github = GithubReleases::at(&base_url, "acme/otelo");
    let release = github.fetch_release(Channel::Stable).unwrap();

    let bytes = github
        .download_verified_asset(&release, "otelo-x.tar.gz")
        .unwrap();

    assert_eq!(bytes, archive);
}

#[test]
fn a_download_that_does_not_match_its_digest_fails() {
    let (_runtime, base_url) = serve_fake_github(release_with_archive(
        b"tampered",
        &write_digest_file(b"original", "otelo-x.tar.gz"),
    ));
    let github = GithubReleases::at(&base_url, "acme/otelo");
    let release = github.fetch_release(Channel::Stable).unwrap();

    let error = github
        .download_verified_asset(&release, "otelo-x.tar.gz")
        .unwrap_err();

    assert!(
        format!("{error:#}").contains("digest mismatch"),
        "{error:#}"
    );
}

#[test]
fn a_missing_asset_names_the_release() {
    let (_runtime, base_url) = serve_fake_github(release_with_archive(b"bytes", b""));
    let github = GithubReleases::at(&base_url, "acme/otelo");
    let release = github.fetch_release(Channel::Stable).unwrap();

    let error = github
        .download_verified_asset(&release, "otelo-y.tar.gz")
        .unwrap_err();

    assert!(
        format!("{error:#}").contains("release v1.2.3 publishes no asset named otelo-y.tar.gz"),
        "{error:#}"
    );
}

#[test]
fn the_canary_release_takes_its_version_from_the_title() {
    let (_runtime, base_url) = serve_fake_github(release_titled("canary", "0.4.1-canary.57"));

    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Canary)
        .unwrap();

    assert_eq!(release.version, parse_version("0.4.1-canary.57"));
    assert_eq!(release.tag, "canary");
    assert_eq!(release.channel, Channel::Canary);
}

#[test]
fn a_canary_title_that_is_not_a_version_fails() {
    let (_runtime, base_url) = serve_fake_github(release_titled("canary", "Canary build"));

    let error = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Canary)
        .map(|_| ())
        .unwrap_err();

    assert!(
        format!("{error:#}")
            .contains(r#"the canary release title "Canary build" is not a version"#),
        "{error:#}"
    );
}

#[test]
fn the_canary_build_installs_when_it_is_not_the_installed_version() {
    let (_runtime, base_url) = serve_fake_github(release_titled("canary", "0.4.1-canary.57"));
    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Canary)
        .unwrap();

    assert_eq!(
        release.choose_step_from(&parse_version("0.4.0")),
        UpdateStep::Install
    );
    assert_eq!(
        release.choose_step_from(&parse_version("0.4.1-canary.56")),
        UpdateStep::Install
    );
    assert_eq!(
        release.choose_step_from(&parse_version("0.4.1-canary.57")),
        UpdateStep::UpToDate
    );
}

#[test]
fn a_canary_build_returns_to_the_older_stable_release() {
    let (_runtime, base_url) = serve_fake_github(release_titled("v0.4.0", "v0.4.0"));
    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Stable)
        .unwrap();

    assert_eq!(
        release.choose_step_from(&parse_version("0.4.1-canary.57")),
        UpdateStep::ReturnToStable
    );
}

#[test]
fn a_canary_build_moves_on_to_the_stable_release_of_its_version() {
    let (_runtime, base_url) = serve_fake_github(release_titled("v0.4.1", "v0.4.1"));
    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Stable)
        .unwrap();

    assert_eq!(
        release.choose_step_from(&parse_version("0.4.1-canary.57")),
        UpdateStep::Install
    );
}

#[test]
fn a_stable_install_stays_on_the_newest_stable_release() {
    let (_runtime, base_url) = serve_fake_github(release_titled("v0.4.0", "v0.4.0"));
    let release = GithubReleases::at(&base_url, "acme/otelo")
        .fetch_release(Channel::Stable)
        .unwrap();

    assert_eq!(
        release.choose_step_from(&parse_version("0.4.0")),
        UpdateStep::UpToDate
    );
    assert_eq!(
        release.choose_step_from(&parse_version("0.3.9")),
        UpdateStep::Install
    );
}
