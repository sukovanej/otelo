//! The endpoints of the services, of one service, and of one of its
//! operations.

use axum::extract::{Path, Query, State};
use serde::Deserialize;
use siner_telemetry::query::{OperationDetail, Service, Services};
use utoipa::IntoParams;

use crate::Api;
use crate::error::{ApiResult, ErrorBody};
use crate::params::{check_step, parse_step};

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
pub async fn services(
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
pub async fn service(
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
pub struct OperationParams {
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
pub async fn operation(
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
