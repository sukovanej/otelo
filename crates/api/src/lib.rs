//! The HTTP query API over the telemetry, and a spec of it for client
//! generators at `/api/openapi.json`.
//!
//! Every query endpoint takes a time range, `since` and `until`, a `limit`,
//! and a query `q` in the language of [`siner_query`], and says when it cut
//! the result. A time is a duration before now (`1h`, `30m`, `2d`) or an RFC
//! 3339 timestamp. The range is capped at the retention.

mod catalog;
mod error;
mod indexes;
mod logs;
mod metrics;
mod params;
mod services;
mod sql;
mod time;
mod traces;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::extract::{MatchedPath, Request as HttpRequest};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use jiff::Timestamp;
use siner_state::State;
use siner_telemetry::{Day, Indexes, Reader};
use tracing::Instrument;
use tracing::field::Empty;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{ApiError, ApiResult};
use crate::time::check_limit;

pub use catalog::{CompletionKind, Completions, SignalName, SuggestionBody};
pub use error::ErrorBody;
pub use indexes::{IndexBody, IndexList};
pub use sql::SqlRequest;
pub use time::{nanos, parse_duration, parse_time};

/// The most rows any query returns.
pub(crate) const MAX_LIMIT: usize = 10_000;

/// How long one query can run.
pub(crate) const TIME_LIMIT: Duration = Duration::from_secs(10);

/// The range of a query that names no `since`.
const DEFAULT_RANGE: &str = "1h";

#[derive(OpenApi)]
#[openapi(info(
    title = "siner",
    description = "Query the logs, traces, and metrics that siner keeps."
))]
struct Spec;

/// Where the day files are, how many days of them there are, and which
/// attributes have an index.
#[derive(Clone)]
pub struct Api {
    pub dir: PathBuf,
    pub retention_days: u16,
    pub indexes: Indexes,
    pub state: Arc<State>,
}

/// The query routes, and the spec that their annotations make.
fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::with_openapi(Spec::openapi())
        .routes(routes!(logs::logs))
        .routes(routes!(logs::log_groups))
        .routes(routes!(traces::spans))
        .routes(routes!(traces::traces))
        .routes(routes!(traces::trace))
        .routes(routes!(metrics::metrics))
        .routes(routes!(metrics::metric))
        .routes(routes!(services::services))
        .routes(routes!(services::service))
        .routes(routes!(services::operation))
        .routes(routes!(sql::sql))
        .routes(routes!(catalog::attributes))
        .routes(routes!(catalog::complete))
        .routes(routes!(indexes::list_indexes))
        .routes(routes!(indexes::add_index, indexes::remove_index))
}

/// The spec of the query API, which `/api/openapi.json` serves and
/// `packages/api` of the web UI generates its types from.
#[must_use]
pub fn spec() -> utoipa::openapi::OpenApi {
    routes().into_openapi()
}

/// The query routes and `/api/openapi.json`, with a span for each request.
pub fn router(api: Api) -> Router {
    let (router, spec) = routes().with_state(api).split_for_parts();
    router
        .route(
            "/api/openapi.json",
            get(move || async move { Json(spec.clone()) }),
        )
        .route_layer(middleware::from_fn(trace))
}

/// Runs the request in a server span named after its route, as the
/// OpenTelemetry HTTP conventions ask.
async fn trace(request: HttpRequest, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| request.uri().path(), MatchedPath::as_str)
        .to_owned();
    let method = request.method().clone();
    let span = tracing::info_span!(
        "request",
        otel.name = format!("{method} {route}"),
        otel.kind = "server",
        http.request.method = %method,
        http.route = route,
        url.path = request.uri().path(),
        url.query = request.uri().query(),
        http.response.status_code = Empty,
    );
    let response = next.run(request).instrument(span.clone()).await;
    // The OpenTelemetry layer keeps an i64 as a number, and a u64 as text.
    span.record(
        "http.response.status_code",
        i64::from(response.status().as_u16()),
    );
    response
}

/// The reader and the limit a handler runs its query with.
pub(crate) struct Request {
    reader: Reader,
    limit: usize,
}

/// The range of the whole retention, for queries that look a thing up.
pub(crate) const WHOLE_RETENTION: Option<i64> = Some(i64::MIN);

impl Api {
    /// Checks the range and the limit, opens a reader, and runs `query` with
    /// it on a blocking thread.
    async fn run<T: Send + 'static>(
        &self,
        range: [Option<String>; 2],
        default_since: Option<i64>,
        limit: (Option<usize>, usize),
        query: impl FnOnce(Request) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        let api = self.clone();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _entered = span.enter();
            let [since, until] = range;
            let (since, until) = api
                .range(since.as_deref(), until.as_deref(), default_since)
                .map_err(|e| ApiError::bad_request(&e))?;
            let limit = check_limit(limit.0, limit.1).map_err(|e| ApiError::bad_request(&e))?;
            let mut reader = Reader::open(&api.dir, since, until)?;
            reader.set_time_limit(TIME_LIMIT)?;
            reader.set_indexes(api.indexes.get());
            query(Request { reader, limit })
        })
        .await
        .context("run the query")?
        .map(Json)
    }

    /// The range in unix nanoseconds, with `since` moved up to the oldest
    /// day the retention keeps. `default_since` stands in for a missing
    /// `since`, and one hour before now when it is `None` too.
    ///
    /// # Errors
    ///
    /// When a time does not parse, or `since` is not before `until`.
    pub fn range(
        &self,
        since: Option<&str>,
        until: Option<&str>,
        default_since: Option<i64>,
    ) -> anyhow::Result<(i64, i64)> {
        let now = nanos(Timestamp::now());
        let until = until.map_or(Ok(now), |text| parse_time(text, now))?;
        let oldest = Day::today()
            .plus(1 - i64::from(self.retention_days.max(1)))
            .start();
        let since = match since {
            Some(text) => parse_time(text, now)?,
            None => default_since.unwrap_or(now - parse_duration(DEFAULT_RANGE)?),
        };
        ensure!(since < until, "since has to be before until");
        Ok((since.max(oldest), until))
    }
}
