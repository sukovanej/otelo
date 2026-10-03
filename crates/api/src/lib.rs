mod catalog;
mod error;
mod indexes;
mod logs;
mod metrics;
mod params;
mod services;
mod time;
mod traces;

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::extract::{MatchedPath, Request};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use jiff::Timestamp;
use otelo_indexed_storage::{RangeQueries, Storage, TimeRange};
use otelo_query::Signal;
use tracing::Instrument;
use tracing::field::Empty;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::{ApiError, ApiResult};
use crate::time::check_limit;

pub use catalog::{
    CompletionKind, Completions, FieldBody, FieldOriginBody, FieldValueBody, SignalName,
    SuggestionBody,
};
pub use error::ErrorBody;
pub use indexes::{IndexBody, IndexList, IndexedSignalName};
pub use time::{convert_to_unix_nanos, parse_duration, parse_time};

pub(crate) const MAX_ROW_LIMIT: usize = 10_000;

pub(crate) const QUERY_TIME_LIMIT: Duration = Duration::from_secs(10);

const HOUR_NS: i64 = 3_600_000_000_000;

// The `signal` query parameters refer to the Signal schema, and no body holds one, so only
// this list puts it in the spec.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "otelo",
        description = "Query the logs, traces, and metrics that otelo keeps."
    ),
    components(schemas(SignalName))
)]
struct OpenApiInfo;

#[derive(Clone)]
pub struct Api {
    pub storage: Arc<dyn Storage>,
}

fn build_query_routes() -> OpenApiRouter<Api> {
    OpenApiRouter::with_openapi(OpenApiInfo::openapi())
        .routes(routes!(logs::list_logs))
        .routes(routes!(logs::list_log_groups))
        .routes(routes!(traces::list_spans))
        .routes(routes!(traces::list_traces))
        .routes(routes!(traces::get_trace))
        .routes(routes!(metrics::list_metrics))
        .routes(routes!(metrics::get_metric_series))
        .routes(routes!(services::list_services))
        .routes(routes!(services::get_service))
        .routes(routes!(services::get_operation))
        .routes(routes!(services::list_calls))
        .routes(routes!(services::get_call))
        .routes(routes!(catalog::list_attribute_keys))
        .routes(routes!(catalog::complete_query))
        .routes(routes!(indexes::list_indexes))
        .routes(routes!(indexes::add_index, indexes::remove_index))
}

#[must_use]
pub fn build_openapi_spec() -> utoipa::openapi::OpenApi {
    build_query_routes().into_openapi()
}

pub fn build_router(api: Api) -> Router {
    let (router, spec) = build_query_routes().with_state(api).split_for_parts();
    router
        .route(
            "/api/openapi.json",
            get(move || async move { Json(spec.clone()) }),
        )
        .route_layer(middleware::from_fn(trace_request))
}

async fn trace_request(request: Request, next: Next) -> Response {
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

pub(crate) struct OpenedRange {
    queries: Box<dyn RangeQueries>,
    range: TimeRange,
}

pub(crate) struct RequestedRange {
    signals: RangeSignals,
    since: Option<String>,
    until: Option<String>,
    default_since: DefaultSince,
}

#[derive(Clone, Copy)]
pub enum RangeSignals {
    One(Signal),
    SpansAndLogs,
}

impl RangeSignals {
    // A range of two signals starts where the shorter retention starts.
    fn oldest_retained_at(self, storage: &dyn Storage) -> i64 {
        match self {
            Self::One(signal) => storage.oldest_retained_at(signal),
            Self::SpansAndLogs => storage
                .oldest_retained_at(Signal::Spans)
                .max(storage.oldest_retained_at(Signal::Logs)),
        }
    }
}

#[derive(Clone, Copy)]
pub enum DefaultSince {
    HourBeforeNow,
    OldestRetained,
}

impl Api {
    async fn run_blocking_query<T: Send + 'static>(
        &self,
        query: impl FnOnce(&Self) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        let api = self.clone();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _entered = span.enter();
            query(&api)
        })
        .await
        .context("run the query")?
        .map(Json)
    }

    async fn run_range_query<T: Send + 'static>(
        &self,
        requested_range: RequestedRange,
        query: impl FnOnce(OpenedRange) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        self.run_blocking_query(move |api| {
            let range = api.resolve_requested_range(
                requested_range.signals,
                requested_range.since.as_deref(),
                requested_range.until.as_deref(),
                requested_range.default_since,
            )?;
            query(api.open_range(range)?)
        })
        .await
    }

    async fn run_limited_range_query<T: Send + 'static>(
        &self,
        requested_range: RequestedRange,
        requested_limit: Option<usize>,
        default_limit: usize,
        query: impl FnOnce(OpenedRange, usize) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        self.run_blocking_query(move |api| {
            let range = api.resolve_requested_range(
                requested_range.signals,
                requested_range.since.as_deref(),
                requested_range.until.as_deref(),
                requested_range.default_since,
            )?;
            let limit = check_limit(requested_limit, default_limit)
                .map_err(|error| ApiError::bad_request(&error))?;
            query(api.open_range(range)?, limit)
        })
        .await
    }

    async fn run_retention_query<T: Send + 'static>(
        &self,
        signal: Signal,
        query: impl FnOnce(OpenedRange) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        self.run_blocking_query(move |api| {
            let range = api.resolve_requested_range(
                RangeSignals::One(signal),
                None,
                None,
                DefaultSince::OldestRetained,
            )?;
            query(api.open_range(range)?)
        })
        .await
    }

    fn resolve_requested_range(
        &self,
        signals: RangeSignals,
        since: Option<&str>,
        until: Option<&str>,
        default_since: DefaultSince,
    ) -> Result<TimeRange, ApiError> {
        self.resolve_range_within_retention(signals, since, until, default_since)
            .map_err(|error| ApiError::bad_request(&error))
    }

    fn open_range(&self, range: TimeRange) -> Result<OpenedRange, ApiError> {
        let queries = self.storage.open_range(range, QUERY_TIME_LIMIT)?;
        Ok(OpenedRange { queries, range })
    }

    pub fn resolve_range_within_retention(
        &self,
        signals: RangeSignals,
        since: Option<&str>,
        until: Option<&str>,
        default_since: DefaultSince,
    ) -> anyhow::Result<TimeRange> {
        let now = convert_to_unix_nanos(Timestamp::now());
        let until = until.map_or(Ok(now), |text| parse_time(text, now))?;
        let since = match (since, default_since) {
            (Some(text), _) => Some(parse_time(text, now)?),
            (None, DefaultSince::HourBeforeNow) => Some(now - HOUR_NS),
            (None, DefaultSince::OldestRetained) => None,
        };
        ensure!(
            since.is_none_or(|since| since < until),
            "since has to be before until"
        );
        let oldest_retained_at = signals.oldest_retained_at(&*self.storage);
        ensure!(
            oldest_retained_at < until,
            "the range ends before the oldest telemetry otelo keeps"
        );
        let start_at = since.map_or(oldest_retained_at, |since| since.max(oldest_retained_at));
        TimeRange::new(start_at, until)
    }
}
