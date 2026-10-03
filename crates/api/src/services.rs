use axum::extract::{Path, Query, State};
use otelo_indexed_storage::query::{Service, Services};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{ApiResult, ErrorBody};
use crate::params::{parse_step_ns, resolve_step};
use crate::{Api, DefaultSince, RangeSignals, RequestedRange};

/// The range, the limit, and the step of the services.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ServicesParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most services to return.
    limit: Option<usize>,
    /// The length of a bucket, such as `1m`. One that makes 60 buckets at
    /// most when missing.
    step: Option<String>,
}

/// The range and the step of one service.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ServiceParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
}

/// The services that sent spans or logs in the range, the most requests
/// first. A request is a span of the server kind with
/// `http.request.method`. Each service has its requests and its logs over
/// the range and in buckets of one step, and the count of its spans.
#[utoipa::path(
    get,
    path = "/api/services",
    params(ServicesParams),
    responses(
        (status = 200, body = Services),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn list_services(
    State(api): State<Api>,
    Query(params): Query<ServicesParams>,
) -> ApiResult<Services> {
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::SpansAndLogs,
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
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
/// step. A service without telemetry in the range has none of either.
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
    api.run_range_query(
        RequestedRange {
            signals: RangeSignals::SpansAndLogs,
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        move |opened| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened.queries.get_service(&name, step_ns)?)
        },
    )
    .await
}
