//! The endpoints that read logs, spans, traces, metrics, and SQL.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use siner_query::Signal;
use siner_telemetry::query::{
    self, LogGroups, Logs, MetricFilter, MetricList, MetricSeries, Spans, SqlResult, Trace, Traces,
};
use utoipa::{IntoParams, ToSchema};

use super::{Api, ApiError, ApiResult, ErrorBody, WHOLE_RETENTION, parse_duration, parse_query};

/// The range, the limit, and the query of a list.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct QueryParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most rows to return.
    limit: Option<usize>,
    /// The records to keep, such as `http.route = "/matches" OR user.id = 7`.
    /// Every record when missing.
    q: Option<String>,
}

/// Log lines, newest first.
#[utoipa::path(
    get,
    path = "/api/logs",
    params(QueryParams),
    responses(
        (status = 200, body = Logs),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn logs(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Logs> {
    let query = parse_query(params.q.as_deref(), Signal::Logs)?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.reader.logs(&query, r.limit)?),
    )
    .await
}

/// Log lines grouped by message template, the largest group first. The
/// template replaces numbers, UUIDs, hex IDs, and quoted strings with
/// placeholders.
#[utoipa::path(
    get,
    path = "/api/logs/groups",
    params(QueryParams),
    responses(
        (status = 200, body = LogGroups),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn log_groups(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<LogGroups> {
    let query = parse_query(params.q.as_deref(), Signal::Logs)?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.reader.log_groups(&query, r.limit)?),
    )
    .await
}

/// Spans, newest first.
#[utoipa::path(
    get,
    path = "/api/spans",
    params(QueryParams),
    responses(
        (status = 200, body = Spans),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn spans(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Spans> {
    let query = parse_query(params.q.as_deref(), Signal::Spans)?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.reader.spans(&query, r.limit)?),
    )
    .await
}

/// Traces with a span that the query keeps, by their root span, newest first.
#[utoipa::path(
    get,
    path = "/api/traces",
    params(QueryParams),
    responses(
        (status = 200, body = Traces),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn traces(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Traces> {
    let query = parse_query(params.q.as_deref(), Signal::Spans)?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.reader.traces(&query, r.limit)?),
    )
    .await
}

/// The range and the limit of a lookup.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct LookupParams {
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
        LookupParams,
    ),
    responses(
        (status = 200, body = Trace),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn trace(
    State(api): State<Api>,
    Path(trace_id): Path<String>,
    Query(params): Query<LookupParams>,
) -> ApiResult<Trace> {
    let id = query::parse_trace_id(&trace_id).map_err(|e| ApiError::bad_request(&e))?;
    api.run(
        [params.since, params.until],
        WHOLE_RETENTION,
        (params.limit, 1000),
        move |r| {
            r.reader
                .trace(id, r.limit)?
                .ok_or_else(|| ApiError::not_found(format!("no spans or logs of trace {trace_id}")))
        },
    )
    .await
}

/// The series that have points in the range and that the query keeps, by
/// name. The query reads `name`, `service`, `kind`, `unit`, the labels, and
/// the resource.
#[utoipa::path(
    get,
    path = "/api/metrics",
    params(QueryParams),
    responses(
        (status = 200, body = MetricList),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn metrics(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<MetricList> {
    let query = parse_query(params.q.as_deref(), Signal::Metrics)?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.reader.metrics(&query, r.limit)?),
    )
    .await
}

/// The range, the limit, the query, and the step of one metric.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct MetricParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most series to return.
    limit: Option<usize>,
    /// The series to keep, by their labels and resource, such as
    /// `state = used`. Every series of the metric when missing.
    q: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
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
pub(super) async fn metric(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<MetricParams>,
) -> ApiResult<MetricSeries> {
    let query = parse_query(params.q.as_deref(), Signal::Metrics)?;
    let step = params
        .step
        .as_deref()
        .map(parse_duration)
        .transpose()
        .map_err(|e| ApiError::bad_request(&e))?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 20),
        move |r| {
            let step_ns =
                step.unwrap_or_else(|| query::default_step(r.reader.since(), r.reader.until()));
            let filter = MetricFilter {
                name,
                query,
                step_ns,
            };
            if step_ns <= 0 || (r.reader.until() - r.reader.since()) / step_ns > query::MAX_BUCKETS
            {
                return Err(ApiError::bad_request(&format!(
                    "the step makes more than {} buckets in the range; raise the step",
                    query::MAX_BUCKETS
                )));
            }
            Ok(r.reader.metric(&filter, r.limit)?)
        },
    )
    .await
}

/// A read-only SQL query.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct SqlRequest {
    /// One `SELECT`. It reads the views `resources`, `logs`, `spans`,
    /// `series`, `points`, `attribute_keys`, and `attribute_values`, which
    /// join the day files with a `day` column in front, or the tables of one
    /// day as `"2026-09-28".logs`.
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
pub(super) async fn sql(
    State(api): State<Api>,
    Json(request): Json<SqlRequest>,
) -> ApiResult<SqlResult> {
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
