use axum::Json;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::{Embed, EmbeddedFile};

use otelo_api::ErrorBody;

// A debug build reads these from disk per request, so a web build needs no new cargo build.
#[derive(Embed)]
#[folder = "../../packages/app/dist/"]
#[allow_missing = true]
struct Assets;

const HASHED_ASSETS_PREFIX: &str = "assets/";

const UI_NOT_BUILT_PAGE: &str =
    "The UI is not built. Run `mise run web:build`, and build otelo again.\n";

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
        // Vite names these files by their content, so they never change.
        let cache = if path.starts_with(HASHED_ASSETS_PREFIX) {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        return respond_with_file(&file, cache, &headers);
    }
    // A missing file under assets/ is a stale page asking for an old build.
    if path.starts_with(HASHED_ASSETS_PREFIX) {
        return StatusCode::NOT_FOUND.into_response();
    }
    Assets::get("index.html").map_or_else(
        || (StatusCode::SERVICE_UNAVAILABLE, UI_NOT_BUILT_PAGE).into_response(),
        |index| respond_with_file(&index, "no-cache", &headers),
    )
}

fn respond_with_file(file: &EmbeddedFile, cache: &'static str, headers: &HeaderMap) -> Response {
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
