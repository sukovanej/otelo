//! The web UI: the build of `packages/app` in the repository, served from the
//! binary.
//!
//! A release build embeds `packages/app/dist`. A debug build reads it from the
//! disk on each request, so `mise run web:build` shows up without a new
//! `cargo build`.
//! Any path that is not a file gets `index.html`, and the UI routes it.

use axum::Json;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

use crate::api::ErrorBody;

#[derive(Embed)]
#[folder = "../../packages/app/dist/"]
#[allow_missing = true]
struct Assets;

/// Vite names the files under `assets/` by their content, so they never change.
const ASSETS_DIR: &str = "assets/";

/// The page when the binary was built without the UI.
const NOT_BUILT: &str = "The UI is not built. Run `mise run web:build`, and build siner again.\n";

/// Serves a file of the UI, or `index.html` for a path that is not one.
pub async fn serve(method: Method, uri: Uri, headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                error: format!("no endpoint at {}", uri.path()),
            }),
        )
            .into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if let Some(file) = Assets::get(path).filter(|_| !path.is_empty()) {
        let cache = if path.starts_with(ASSETS_DIR) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return file_response(&file, cache, &headers);
    }
    // A missing file under assets/ is a stale page asking for an old build.
    if path.starts_with(ASSETS_DIR) {
        return StatusCode::NOT_FOUND.into_response();
    }
    Assets::get("index.html").map_or_else(
        || (StatusCode::SERVICE_UNAVAILABLE, NOT_BUILT).into_response(),
        |index| file_response(&index, "no-cache", &headers),
    )
}

/// The file with its type, and a 304 when the client has this version.
fn file_response(file: &EmbeddedFile, cache: &'static str, headers: &HeaderMap) -> Response {
    let etag = format!("\"{}\"", hex(&file.metadata.sha256_hash()[..16]));
    let cache = (CACHE_CONTROL, HeaderValue::from_static(cache));
    if headers
        .get(IF_NONE_MATCH)
        .is_some_and(|value| value.as_bytes() == etag.as_bytes())
    {
        return (StatusCode::NOT_MODIFIED, [cache]).into_response();
    }
    let content_type = HeaderValue::from_str(file.metadata.mimetype())
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let etag = HeaderValue::from_str(&etag).expect("hex digits in quotes are a header value");
    (
        [(CONTENT_TYPE, content_type), (ETAG, etag), cache],
        file.data.clone(),
    )
        .into_response()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
