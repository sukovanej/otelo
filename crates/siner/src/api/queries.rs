//! The endpoints that read logs, spans, traces, metrics, services, and SQL.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use siner_query::Signal;
use siner_telemetry::query::{
    self, LogGroups, Logs, MetricFilter, MetricList, MetricSeries, OperationDetail, Service,
    Services, Spans, SqlResult, Trace, Traces,
};
use utoipa::{IntoParams, ToSchema};

use super::{
    Api, ApiError, ApiResult, ErrorBody, Request, WHOLE_RETENTION, parse_duration, parse_query,
};

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
    let step = parse_step(params.step.as_deref())?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 20),
        move |r| {
            let filter = MetricFilter {
                name,
                query,
                step_ns: check_step(&r, step, 120)?,
            };
            Ok(r.reader.metric(&filter, r.limit)?)
        },
    )
    .await
}

/// A step in nanoseconds, or `None` when `text` is missing.
fn parse_step(text: Option<&str>) -> Result<Option<i64>, ApiError> {
    text.map(parse_duration)
        .transpose()
        .map_err(|e| ApiError::bad_request(&e))
}

/// The step, or the smallest round one that makes `buckets` buckets at most
/// in the range, when it makes no more than [`query::MAX_BUCKETS`].
fn check_step(r: &Request, step: Option<i64>, buckets: i64) -> Result<i64, ApiError> {
    let (since, until) = (r.reader.since(), r.reader.until());
    let step_ns = step.unwrap_or_else(|| query::step_for(since, until, buckets));
    if step_ns <= 0 || (until - since) / step_ns > query::MAX_BUCKETS {
        return Err(ApiError::bad_request(&format!(
            "the step makes more than {} buckets in the range; raise the step",
            query::MAX_BUCKETS
        )));
    }
    Ok(step_ns)
}

/// The range, the limit, and the step of the services.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct ServiceParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most services, or the most operations of one service, to return.
    limit: Option<usize>,
    /// The length of a bucket, such as `1m`. One that makes 60 buckets at
    /// most for the list and 120 for one service when missing.
    step: Option<String>,
}

/// The services that sent spans or logs in the range, the most requests
/// first. A request is a span that enters the service: a root span, or a
/// span of the server or the consumer kind. Each service has its requests and
/// its logs over the range and in buckets of one step.
#[utoipa::path(
    get,
    path = "/api/services",
    params(ServiceParams),
    responses(
        (status = 200, body = Services),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn services(
    State(api): State<Api>,
    Query(params): Query<ServiceParams>,
) -> ApiResult<Services> {
    let step = parse_step(params.step.as_deref())?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| {
            let step_ns = check_step(&r, step, 60)?;
            Ok(r.reader.services(step_ns, r.limit)?)
        },
    )
    .await
}

/// One service: its requests and logs over the range and in buckets of one
/// step, and its requests by span name, the most first. A service without
/// telemetry in the range has none of either.
#[utoipa::path(
    get,
    path = "/api/services/{name}",
    params(
        ("name" = String, Path, description = "The name of the service"),
        ServiceParams,
    ),
    responses(
        (status = 200, body = Service),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn service(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<ServiceParams>,
) -> ApiResult<Service> {
    let step = parse_step(params.step.as_deref())?;
    api.run(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| {
            let step_ns = check_step(&r, step, 120)?;
            Ok(r.reader.service(&name, step_ns, r.limit)?)
        },
    )
    .await
}

/// The range, the step, and the operation of one service.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct OperationParams {
    /// The span name of the operation, such as `GET /users/{id}`.
    operation: String,
    /// The OpenTelemetry span kind of the operation, such as 2 for server.
    kind: i32,
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
}

/// One operation of a service: its requests over the range and in buckets of
/// one step, and the attributes of its newest request. An operation without
/// requests in the range has none.
#[utoipa::path(
    get,
    path = "/api/services/{name}/operation",
    params(
        ("name" = String, Path, description = "The name of the service"),
        OperationParams,
    ),
    responses(
        (status = 200, body = OperationDetail),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn operation(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<OperationParams>,
) -> ApiResult<OperationDetail> {
    let step = parse_step(params.step.as_deref())?;
    let (operation, kind) = (params.operation, params.kind);
    api.run([params.since, params.until], None, (None, 1), move |r| {
        let step_ns = check_step(&r, step, 120)?;
        Ok(r.reader.operation(&name, &operation, kind, step_ns)?)
    })
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
