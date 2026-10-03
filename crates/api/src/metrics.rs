use std::num::NonZeroUsize;

use axum::extract::{Path, Query, State};
use otelo_indexed_storage::query::{
    Grouping, GroupingField, MetricFilter, MetricList, MetricSeries, Resolution,
};
use otelo_query::Signal;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::params::{QueryParams, parse_query, parse_step_ns, resolve_step};
use crate::{Api, DefaultSince, RangeSignals, RequestedRange};

/// The series that have points in the range and that the query keeps, by
/// name. The query reads `name`, `service`, `kind`, `unit`, the attributes, and
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
pub async fn list_metrics(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<MetricList> {
    let query = parse_query(params.query.as_deref(), Signal::Metrics)?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Metrics),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        100,
        move |opened, limit| {
            let resolution = Resolution::choose_for_range_length(opened.range);
            Ok(opened.queries.list_metrics(&query, resolution, limit)?)
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
    /// The series to keep, by their attributes and resource, such as
    /// `state = used`. Every series of the metric when missing.
    #[serde(rename = "q")]
    #[param(rename = "q")]
    query: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
    /// Which points to read: `raw`, `1m`, or `1h`. When missing, the raw
    /// points for a range of 6 hours at most, the summaries by the minute
    /// for one of 14 days at most, and the summaries by the hour for a longer
    /// one, or the next of them that is still kept where the range starts.
    resolution: Option<Resolution>,
    /// The names to group the series by, separated by commas: attributes,
    /// `service`, or `resource.<key>`, such as `http.route,resource.host.name`.
    /// The series with the same values of them combine into one group. Each
    /// series is its own group when missing.
    by: Option<String>,
    /// Keep the N groups with the highest value over the range, and combine
    /// the rest into one group `other`.
    #[param(value_type = Option<usize>, minimum = 1)]
    top: Option<NonZeroUsize>,
}

fn parse_grouping_fields(text: Option<&str>) -> Result<Vec<GroupingField>, ApiError> {
    text.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            name.parse()
                .map_err(|error: String| ApiError::bad_request(&error))
        })
        .collect()
}

/// The series of one metric, or the groups of them, each in buckets of one
/// step with the count, the minimum, the average, the maximum, and the last
/// value, the rate of a counter, and the distribution of a histogram.
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
pub async fn get_metric_series(
    State(api): State<Api>,
    Path(name): Path<String>,
    Query(params): Query<MetricParams>,
) -> ApiResult<MetricSeries> {
    let query = parse_query(params.query.as_deref(), Signal::Metrics)?;
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    let grouping = Grouping {
        by: parse_grouping_fields(params.by.as_deref())?,
        top: params.top,
    };
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Metrics),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        20,
        move |opened, limit| {
            let filter = MetricFilter {
                name,
                query,
                step_ns: resolve_step(opened.range, requested_step_ns, 120)?,
                resolution: params
                    .resolution
                    .unwrap_or_else(|| Resolution::choose_for_range_length(opened.range)),
                grouping,
            };
            Ok(opened.queries.get_metric_series(&filter, limit)?)
        },
    )
    .await
}
