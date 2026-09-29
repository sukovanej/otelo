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

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::extract::{MatchedPath, Request as HttpRequest};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use jiff::Timestamp;
use siner_storage::{RangeQueries, Storage, TimeRange};
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

pub(crate) const MAX_LIMIT: usize = 10_000;

pub(crate) const QUERY_TIME_LIMIT: Duration = Duration::from_secs(10);

const DEFAULT_SINCE: &str = "1h";

#[derive(OpenApi)]
#[openapi(info(
    title = "siner",
    description = "Query the logs, traces, and metrics that siner keeps."
))]
struct Spec;

#[derive(Clone)]
pub struct Api {
    pub storage: Arc<dyn Storage>,
}

fn build_query_routes() -> OpenApiRouter<Api> {
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
        .routes(routes!(services::calls))
        .routes(routes!(services::call))
        .routes(routes!(sql::sql))
        .routes(routes!(catalog::attributes))
        .routes(routes!(catalog::complete))
        .routes(routes!(indexes::list_indexes))
        .routes(routes!(indexes::add_index, indexes::remove_index))
}

#[must_use]
pub fn spec() -> utoipa::openapi::OpenApi {
    build_query_routes().into_openapi()
}

pub fn router(api: Api) -> Router {
    let (router, spec) = build_query_routes().with_state(api).split_for_parts();
    router
        .route(
            "/api/openapi.json",
            get(move || async move { Json(spec.clone()) }),
        )
        .route_layer(middleware::from_fn(trace_request))
}

async fn trace_request(request: HttpRequest, next: Next) -> Response {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| request.uri().path(), MatchedPath::as_str)
        .to_owned();
    let method = request.method().clone();
    // The OpenTelemetry HTTP conventions name a server span after its method and route.
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

pub(crate) struct Request {
    queries: Box<dyn RangeQueries>,
    range: TimeRange,
    limit: usize,
}

pub(crate) const WHOLE_RETENTION: Option<i64> = Some(i64::MIN);

impl Api {
    async fn run_range_query<T: Send + 'static>(
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
            let range = api
                .resolve_range(since.as_deref(), until.as_deref(), default_since)
                .map_err(|e| ApiError::bad_request(&e))?;
            let limit = check_limit(limit.0, limit.1).map_err(|e| ApiError::bad_request(&e))?;
            let queries = api.storage.open_range(range, QUERY_TIME_LIMIT)?;
            query(Request {
                queries,
                range,
                limit,
            })
        })
        .await
        .context("run the query")?
        .map(Json)
    }

    pub fn resolve_range(
        &self,
        since: Option<&str>,
        until: Option<&str>,
        default_since: Option<i64>,
    ) -> anyhow::Result<TimeRange> {
        let now = nanos(Timestamp::now());
        let until = until.map_or(Ok(now), |text| parse_time(text, now))?;
        let since = match since {
            Some(text) => parse_time(text, now)?,
            None => default_since.unwrap_or(now - parse_duration(DEFAULT_SINCE)?),
        };
        ensure!(since < until, "since has to be before until");
        let oldest_retained_at = self.storage.oldest_retained_at();
        ensure!(
            oldest_retained_at < until,
            "the range ends before the oldest telemetry siner keeps"
        );
        TimeRange::new(since.max(oldest_retained_at), until)
    }
}
