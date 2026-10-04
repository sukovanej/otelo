use axum::extract::{Query, State};
use otelo_indexed_storage::query::{
    LogCounts, LogGroupingField, LogGroups, Logs, PageRequest, RankOrder,
};
use otelo_query::Signal;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{ApiResult, ErrorBody};
use crate::params::{
    QueryParams, parse_field_list, parse_page_cursor, parse_query, parse_step_ns, resolve_step,
};
use crate::{Api, DefaultSince, RangeSignals, RequestedRange, RowLimits};

/// The range, the page, and the query of a list of log lines.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LogListParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most rows to return, from 1 to 1000.
    limit: Option<usize>,
    /// The `next` of the page before, to return the page after it. The first
    /// page when missing.
    after: Option<String>,
    /// The lines to keep, such as `level >= warn service = "api"`. Every line
    /// when missing.
    #[serde(rename = "q")]
    #[param(rename = "q")]
    query: Option<String>,
}

/// Log lines, newest first.
#[utoipa::path(
    get,
    path = "/api/logs",
    params(LogListParams),
    responses(
        (status = 200, body = Logs),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn list_logs(
    State(api): State<Api>,
    Query(params): Query<LogListParams>,
) -> ApiResult<Logs> {
    let query = parse_query(params.query.as_deref(), Signal::Logs)?;
    let after = parse_page_cursor(params.after.as_deref())?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Logs),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        RowLimits::up_to_max_page_rows(100),
        move |opened, limit| {
            Ok(opened
                .queries
                .list_logs(&query, PageRequest { after, limit })?)
        },
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
pub async fn list_log_groups(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<LogGroups> {
    let query = parse_query(params.query.as_deref(), Signal::Logs)?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Logs),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        RowLimits::up_to_max_rows(50),
        move |opened, limit| Ok(opened.queries.list_log_groups(&query, limit)?),
    )
    .await
}

/// The range, the limit, the query, the grouping, and the step of log counts.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LogCountParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most groups to return.
    limit: Option<usize>,
    /// The lines to count, such as `level >= warn`. Every line when missing.
    #[serde(rename = "q")]
    #[param(rename = "q")]
    query: Option<String>,
    /// The names to group the lines by, separated by commas: attributes,
    /// `service`, `level`, or `resource.<key>`. All lines are one group when
    /// missing.
    by: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
    /// Which groups come first and stay within the limit: the ones with the
    /// `highest` count of lines or the `lowest`. `highest` when missing.
    order: Option<RankOrder>,
}

/// The count of the log lines that the query keeps, over the range and in
/// buckets of one step, and grouped by the values of the `by` names, ranked
/// by their count of lines in `order`. Each group has its own buckets.
#[utoipa::path(
    get,
    path = "/api/logs/counts",
    params(LogCountParams),
    responses(
        (status = 200, body = LogCounts),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn count_logs(
    State(api): State<Api>,
    Query(params): Query<LogCountParams>,
) -> ApiResult<LogCounts> {
    let query = parse_query(params.query.as_deref(), Signal::Logs)?;
    let by: Vec<LogGroupingField> = parse_field_list(params.by.as_deref())?;
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    let order = params.order.unwrap_or_default();
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Logs),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        RowLimits::up_to_max_rows(50),
        move |opened, limit| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened
                .queries
                .count_logs(&query, &by, order, step_ns, limit)?)
        },
    )
    .await
}
