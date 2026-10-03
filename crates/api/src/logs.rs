use axum::extract::{Query, State};
use otelo_indexed_storage::query::{LogGroups, Logs};
use otelo_query::Signal;

use crate::error::{ApiResult, ErrorBody};
use crate::params::{QueryParams, parse_query};
use crate::{Api, DefaultSince};

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
pub async fn list_logs(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Logs> {
    let query = parse_query(params.query.as_deref(), Signal::Logs)?;
    api.run_limited_range_query(
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        params.limit,
        100,
        move |opened, limit| Ok(opened.queries.list_logs(&query, limit)?),
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
        params.since,
        params.until,
        DefaultSince::HourBeforeNow,
        params.limit,
        50,
        move |opened, limit| Ok(opened.queries.list_log_groups(&query, limit)?),
    )
    .await
}
