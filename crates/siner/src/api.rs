//! The HTTP query API over the telemetry, and a spec of it for client
//! generators at `/api/openapi.json`.
//!
//! Every endpoint takes a time range, `since` and `until`, and a `limit`, and
//! says when it cut the result. A time is a duration before now (`1h`, `30m`,
//! `2d`) or an RFC 3339 timestamp. The range is capped at the retention.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use jiff::{SpanRelativeTo, Timestamp};
use serde::{Deserialize, Serialize};
use siner_telemetry::query::{
    self, LogFilter, LogGroups, Logs, MetricFilter, MetricList, MetricSeries, SqlResult, Trace,
    TraceFilter, Traces,
};
use siner_telemetry::{Day, Reader};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// The most rows any query returns.
const MAX_LIMIT: usize = 10_000;

/// How long one query can run.
const TIME_LIMIT: Duration = Duration::from_secs(10);

/// The range of a query that names no `since`.
const DEFAULT_RANGE: &str = "1h";

#[derive(OpenApi)]
#[openapi(info(
    title = "siner",
    description = "Query the logs, traces, and metrics that siner keeps."
))]
struct Spec;

/// Where the day files are and how many days of them there are.
#[derive(Clone)]
pub struct Api {
    pub dir: PathBuf,
    pub retention_days: u16,
}

/// The query routes and `/api/openapi.json`.
pub fn router(api: Api) -> Router {
    let (router, spec) = OpenApiRouter::with_openapi(Spec::openapi())
        .routes(routes!(logs))
        .routes(routes!(log_groups))
        .routes(routes!(traces))
        .routes(routes!(trace))
        .routes(routes!(metrics))
        .routes(routes!(metric))
        .routes(routes!(sql))
        .with_state(api)
        .split_for_parts();
    router.route(
        "/api/openapi.json",
        get(move || async move { Json(spec.clone()) }),
    )
}

/// The body of every error response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(error: &anyhow::Error) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: format!("{error:#}"),
        }
    }

    const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        if siner_telemetry::timed_out(&error) {
            return Self {
                status: StatusCode::BAD_REQUEST,
                message: format!(
                    "the query ran longer than {} s; narrow the range or the filters",
                    TIME_LIMIT.as_secs()
                ),
            };
        }
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{error:#}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

/// Checks the range and the limit and opens a reader, so that each handler
/// only has to run its query.
struct Request {
    reader: Reader,
    limit: usize,
}

impl Api {
    /// Runs `query` on a blocking thread with a reader of the range.
    async fn run<T: Send + 'static>(
        &self,
        range: [Option<String>; 2],
        default_since: Option<i64>,
        limit: (Option<usize>, usize),
        query: impl FnOnce(Request) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let [since, until] = range;
            let (since, until) = api
                .range(since.as_deref(), until.as_deref(), default_since)
                .map_err(|e| ApiError::bad_request(&e))?;
            let limit = check_limit(limit.0, limit.1).map_err(|e| ApiError::bad_request(&e))?;
            let reader = Reader::open(&api.dir, since, until)?;
            reader.set_time_limit(TIME_LIMIT)?;
            query(Request { reader, limit })
        })
        .await
        .context("run the query")?
        .map(Json)
    }

    /// The range in unix nanoseconds, with `since` moved up to the oldest
    /// day the retention keeps.
    fn range(
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

fn check_limit(limit: Option<usize>, default: usize) -> anyhow::Result<usize> {
    let limit = limit.unwrap_or(default);
    ensure!(
        (1..=MAX_LIMIT).contains(&limit),
        "the limit is {limit}, and it has to be from 1 to {MAX_LIMIT}"
    );
    Ok(limit)
}

fn nanos(ts: Timestamp) -> i64 {
    i64::try_from(ts.as_nanosecond()).unwrap_or(i64::MAX)
}

/// A duration before `now`, such as `1h`, or an RFC 3339 timestamp.
fn parse_time(text: &str, now: i64) -> anyhow::Result<i64> {
    if let Ok(ts) = text.parse::<Timestamp>() {
        return Ok(nanos(ts));
    }
    let ago = parse_duration(text).with_context(|| {
        format!("{text:?} is neither a duration such as 1h nor an RFC 3339 timestamp")
    })?;
    Ok(now.saturating_sub(ago))
}

/// A duration such as `500ms`, `1h`, or `2d`, in nanoseconds.
fn parse_duration(text: &str) -> anyhow::Result<i64> {
    let span: jiff::Span = text
        .parse()
        .with_context(|| format!("{text:?} is not a duration such as 500ms, 1h, or 2d"))?;
    let duration = span.to_duration(SpanRelativeTo::days_are_24_hours())?;
    ensure!(!duration.is_negative(), "{text:?} is negative");
    Ok(i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
}

fn parse_trace_id(text: &str) -> Result<[u8; 16], ApiError> {
    query::parse_trace_id(text).map_err(|e| ApiError::bad_request(&e))
}

/// The filters of a log query.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct LogParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most lines, or groups, to return.
    limit: Option<usize>,
    /// The service that wrote the line.
    service: Option<String>,
    /// The lowest severity: `trace`, `debug`, `info`, `warn`, `error`,
    /// `fatal`, or an OpenTelemetry severity number.
    severity: Option<String>,
    /// Words that every line has to contain.
    search: Option<String>,
    /// The trace ID of the lines, as 32 hex digits.
    trace_id: Option<String>,
}

impl LogParams {
    fn filter(&self) -> Result<LogFilter, ApiError> {
        Ok(LogFilter {
            service: self.service.clone(),
            min_severity: self
                .severity
                .as_deref()
                .map(query::parse_severity)
                .transpose()
                .map_err(|e| ApiError::bad_request(&e))?,
            search: self.search.clone(),
            trace_id: self.trace_id.as_deref().map(parse_trace_id).transpose()?,
        })
    }
}

/// Log lines, newest first.
#[utoipa::path(
    get,
    path = "/api/logs",
    params(LogParams),
    responses(
        (status = 200, body = Logs),
        (status = 400, body = ErrorBody),
    ),
)]
async fn logs(State(api): State<Api>, Query(params): Query<LogParams>) -> ApiResult<Logs> {
    let filter = params.filter()?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.reader.logs(&filter, r.limit)?),
    )
    .await
}

/// Log lines grouped by message template, the largest group first. The
/// template replaces numbers, UUIDs, hex IDs, and quoted strings with
/// placeholders.
#[utoipa::path(
    get,
    path = "/api/logs/groups",
    params(LogParams),
    responses(
        (status = 200, body = LogGroups),
        (status = 400, body = ErrorBody),
    ),
)]
async fn log_groups(
    State(api): State<Api>,
    Query(params): Query<LogParams>,
) -> ApiResult<LogGroups> {
    let filter = params.filter()?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.reader.log_groups(&filter, r.limit)?),
    )
    .await
}

/// The filters of a trace query.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct TraceParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most traces to return.
    limit: Option<usize>,
    /// The service of the root span.
    service: Option<String>,
    /// Text that the name of the root span contains.
    name: Option<String>,
    /// The shortest root span to keep, such as `500ms`.
    min_duration: Option<String>,
    /// Keep only the traces with a failed span.
    errors: Option<bool>,
}

/// Traces by their root span, newest first.
#[utoipa::path(
    get,
    path = "/api/traces",
    params(TraceParams),
    responses(
        (status = 200, body = Traces),
        (status = 400, body = ErrorBody),
    ),
)]
async fn traces(State(api): State<Api>, Query(params): Query<TraceParams>) -> ApiResult<Traces> {
    let filter = TraceFilter {
        service: params.service,
        name: params.name,
        min_duration_ns: params
            .min_duration
            .as_deref()
            .map(parse_duration)
            .transpose()
            .map_err(|e| ApiError::bad_request(&e))?,
        errors: params.errors.unwrap_or(false),
    };
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.reader.traces(&filter, r.limit)?),
    )
    .await
}

/// The range of a single trace or of an SQL query.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct RangeParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. The whole retention when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most spans, and the most logs, to return.
    limit: Option<usize>,
}

/// One trace: its spans by start time, and the logs that carry its ID.
#[utoipa::path(
    get,
    path = "/api/traces/{trace_id}",
    params(
        ("trace_id" = String, Path, description = "The trace ID as 32 hex digits"),
        RangeParams,
    ),
    responses(
        (status = 200, body = Trace),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
async fn trace(
    State(api): State<Api>,
    Path(trace_id): Path<String>,
    Query(params): Query<RangeParams>,
) -> ApiResult<Trace> {
    let id = parse_trace_id(&trace_id)?;
    api.run(
        [params.since, params.until],
        Some(i64::MIN),
        (params.limit, 1000),
        move |r| {
            r.reader
                .trace(id, r.limit)?
                .ok_or_else(|| ApiError::not_found(format!("no spans or logs of trace {trace_id}")))
        },
    )
    .await
}

/// The filters of a metric query.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct MetricParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most series to return.
    limit: Option<usize>,
    /// The service of the series.
    service: Option<String>,
    /// Labels the series must have, as `name=value` pairs split by commas.
    labels: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
}

/// The series that have points in the range, by name.
#[utoipa::path(
    get,
    path = "/api/metrics",
    params(MetricParams),
    responses(
        (status = 200, body = MetricList),
        (status = 400, body = ErrorBody),
    ),
)]
async fn metrics(
    State(api): State<Api>,
    Query(params): Query<MetricParams>,
) -> ApiResult<MetricList> {
    let service = params.service;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.reader.metrics(service.as_deref(), r.limit)?),
    )
    .await
}

/// The series of one metric, each in buckets of one step with the count, the
/// minimum, the average, the maximum, and the last value.
#[utoipa::path(
    get,
    path = "/api/metrics/{name}",
    params(
        ("name" = String, Path, description = "The name of the metric"),
        MetricParams,
    ),
    responses(
        (status = 200, body = MetricSeries),
        (status = 400, body = ErrorBody),
    ),
)]
async fn metric(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<MetricParams>,
) -> ApiResult<MetricSeries> {
    let labels = parse_labels(params.labels.as_deref().unwrap_or_default())
        .map_err(|e| ApiError::bad_request(&e))?;
    let step = params
        .step
        .as_deref()
        .map(parse_duration)
        .transpose()
        .map_err(|e| ApiError::bad_request(&e))?;
    let service = params.service;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 20),
        move |r| {
            let filter = MetricFilter {
                name,
                service,
                labels,
                step_ns: step
                    .unwrap_or_else(|| query::default_step(r.reader.since(), r.reader.until())),
            };
            r.reader
                .metric(&filter, r.limit)
                .map_err(|e| ApiError::bad_request(&e))
        },
    )
    .await
}

fn parse_labels(text: &str) -> anyhow::Result<Vec<(String, String)>> {
    text.split(',')
        .filter(|pair| !pair.trim().is_empty())
        .map(|pair| {
            let (name, value) = pair
                .split_once('=')
                .with_context(|| format!("the label {pair:?} is not name=value"))?;
            Ok((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// A read-only SQL query.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SqlRequest {
    /// One `SELECT`. It reads the views `resources`, `logs`, `spans`,
    /// `series`, and `points`, which join the day files with a `day` column in
    /// front, or the tables of one day as `"2026-09-28".logs`.
    pub sql: String,
    /// The start of the range, which chooses the day files: a duration before
    /// now, such as `1h`, or an RFC 3339 timestamp. One hour before `until`
    /// when missing.
    pub since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    pub until: Option<String>,
    /// The most rows to return.
    pub limit: Option<usize>,
}

/// Runs a read-only SQL query over the day files of the range.
#[utoipa::path(
    post,
    path = "/api/sql",
    request_body = SqlRequest,
    responses(
        (status = 200, body = SqlResult),
        (status = 400, body = ErrorBody),
    ),
)]
async fn sql(State(api): State<Api>, Json(request): Json<SqlRequest>) -> ApiResult<SqlResult> {
    let sql = request.sql;
    api.run(
        [request.since, request.until],
        None,
        (request.limit, 100),
        move |r| {
            r.reader.sql(&sql, r.limit).map_err(|e| {
                if siner_telemetry::timed_out(&e) {
                    e.into()
                } else {
                    ApiError::bad_request(&e)
                }
            })
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600 * 1_000_000_000;

    #[test]
    fn parses_durations_and_timestamps() {
        let now = nanos(Timestamp::now());
        assert_eq!(parse_time("1h", now).unwrap(), now - HOUR);
        assert_eq!(parse_time("2d", now).unwrap(), now - 48 * HOUR);
        assert_eq!(parse_duration("500ms").unwrap(), 500_000_000);
        assert_eq!(
            parse_time("2026-09-28T00:00:00Z", now).unwrap(),
            nanos("2026-09-28T00:00:00Z".parse().unwrap())
        );
        assert!(parse_time("yesterday", now).is_err());
    }

    #[test]
    fn caps_the_range_at_the_retention() {
        let api = Api {
            dir: PathBuf::new(),
            retention_days: 7,
        };
        let (since, _) = api.range(Some("30d"), None, None).unwrap();
        assert_eq!(since, Day::today().plus(-6).start());
        assert!(api.range(Some("1h"), Some("2h"), None).is_err());
    }

    #[test]
    fn parses_labels() {
        assert_eq!(
            parse_labels("state=used, cpu=0").unwrap(),
            [("state".into(), "used".into()), ("cpu".into(), "0".into())]
        );
        assert!(parse_labels("used").is_err());
    }
}
