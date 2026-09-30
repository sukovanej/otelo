use axum::extract::{Path, Query, State};
use otelo_storage::SpanKind;
use otelo_storage::query::{
    CallDetail, Calls, OperationDetail, Service, Services, TargetKey, TargetType,
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{ApiResult, ErrorBody};
use crate::params::{parse_step_ns, resolve_step};
use crate::{Api, DefaultSince};

/// The range, the limit, and the step of the services.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ServiceParams {
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
pub async fn list_services(
    State(api): State<Api>,
    Query(params): Query<ServiceParams>,
) -> ApiResult<Services> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    api.run_limited_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        params.limit,
        100,
        move |opened, limit| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 60)?;
            Ok(opened.queries.list_services(step_ns, limit)?)
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
pub async fn get_service(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<ServiceParams>,
) -> ApiResult<Service> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    api.run_limited_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        params.limit,
        50,
        move |opened, limit| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened.queries.get_service(&name, step_ns, limit)?)
        },
    )
    .await
}

/// The range, the step, and the operation of one service.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct OperationParams {
    /// The span name of the operation, such as `GET /users/{id}`.
    operation: String,
    /// The OpenTelemetry span kind of the operation, such as 2 for server.
    #[param(value_type = i32)]
    kind: SpanKind,
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
pub async fn get_operation(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<OperationParams>,
) -> ApiResult<OperationDetail> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    let (operation, kind) = (params.operation, params.kind);
    api.run_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        move |opened| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened
                .queries
                .get_operation(&name, &operation, kind, step_ns)?)
        },
    )
    .await
}

/// The calls a service makes, by what they go to: a span of the client or
/// the producer kind, or of a database system. A target is a database, a
/// host, an RPC service, or a message destination, read from the
/// OpenTelemetry attributes of the call. Each target has its calls over the
/// range and in buckets of one step, and by span name and kind, the most time
/// first. What a call does is its query with the values taken out, the
/// method and the path of an HTTP request with its ids taken out, or else
/// its span name.
#[utoipa::path(
    get,
    path = "/api/services/{name}/calls",
    params(
        ("name" = String, Path, description = "The name of the service"),
        ServiceParams,
    ),
    responses(
        (status = 200, body = Calls),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn list_calls(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<ServiceParams>,
) -> ApiResult<Calls> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    api.run_limited_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        params.limit,
        50,
        move |opened, limit| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened.queries.list_calls(&name, step_ns, limit)?)
        },
    )
    .await
}

/// The range, the step, the target, and what one call does.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CallParams {
    /// What the target is.
    #[serde(rename = "type")]
    #[param(rename = "type", inline)]
    target_type: TargetType,
    /// The system of the target, such as `postgresql`. Missing for a
    /// target without one.
    system: Option<String>,
    /// The name of the target, such as a database or a host. Missing for a
    /// target without one.
    target: Option<String>,
    /// What the calls do, such as `SELECT * FROM users WHERE id = ?`, as
    /// `/api/services/{name}/calls` names it.
    summary: String,
    /// The OpenTelemetry span kind of the call, such as 3 for client.
    #[param(value_type = i32)]
    kind: SpanKind,
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
}

/// The calls of a service to one target that do one thing, of one kind:
/// over the range and in buckets of one step, the attributes of the newest
/// one, and the newest 50. A call without spans in the range has none.
#[utoipa::path(
    get,
    path = "/api/services/{name}/call",
    params(
        ("name" = String, Path, description = "The name of the service"),
        CallParams,
    ),
    responses(
        (status = 200, body = CallDetail),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn get_call(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<CallParams>,
) -> ApiResult<CallDetail> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    let target = TargetKey {
        target_type: params.target_type,
        system: params.system,
        name: params.target,
    };
    let (summary, kind) = (params.summary, params.kind);
    api.run_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        move |opened| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened
                .queries
                .get_call(&name, &target, &summary, kind, step_ns)?)
        },
    )
    .await
}
