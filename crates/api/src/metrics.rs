use axum::extract::{Path, Query, State};
use otelo_query::Signal;
use otelo_storage::query::{MetricFilter, MetricList, MetricSeries, Resolution};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::Api;
use crate::error::{ApiResult, ErrorBody};
use crate::params::{QueryParams, parse_query, parse_step_ns, resolve_step};

/// The series that have points in the range and that the query keeps, by
/// name. The query reads `name`, `service`, `kind`, `unit`, the labels, and
/// the resource. A range of metrics can go back 90 days, further than the
/// logs and the spans.
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
    let retention = api.storage.metric_retention();
    api.run_metric_range_query(
        [params.since, params.until],
        (params.limit, 100),
        move |r| {
            let resolution = Resolution::finest_kept_for(r.range, retention);
            Ok(r.queries.metrics(&query, resolution, r.limit)?)
        },
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
    /// Which points to read: `raw`, `1m`, or `1h`. When missing, the raw
    /// points for a range of 6 hours at most, the summaries by the minute
    /// for one of 14 days at most, and the summaries by the hour for a longer
    /// one, or the next of them that is still kept where the range starts.
    resolution: Option<Resolution>,
}

/// The series of one metric, each in buckets of one step with the count, the
/// minimum, the average, the maximum, and the last value, the rate of a
/// counter, and the distribution of a histogram.
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
    let step = parse_step_ns(params.step.as_deref())?;
    let retention = api.storage.metric_retention();
    api.run_metric_range_query([params.since, params.until], (params.limit, 20), move |r| {
        let filter = MetricFilter {
            name,
            query,
            step_ns: resolve_step(&r, step, 120)?,
            resolution: params
                .resolution
                .unwrap_or_else(|| Resolution::finest_kept_for(r.range, retention)),
        };
        Ok(r.queries.metric(&filter, r.limit)?)
    })
    .await
}
