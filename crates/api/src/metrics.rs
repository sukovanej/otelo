//! The endpoints of the metric series and of one metric in buckets.

use axum::extract::{Path, Query, State};
use serde::Deserialize;
use siner_query::Signal;
use siner_telemetry::query::{MetricFilter, MetricList, MetricSeries};
use utoipa::IntoParams;

use crate::Api;
use crate::error::{ApiResult, ErrorBody};
use crate::params::{QueryParams, check_step, parse_query, parse_step};

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
pub async fn metrics(
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
pub struct MetricParams {
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
pub async fn metric(
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
