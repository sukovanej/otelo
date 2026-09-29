use axum::extract::{Query, State};
use otelo_query::Signal;
use otelo_storage::query::{LogGroups, Logs};

use crate::Api;
use crate::error::{ApiResult, ErrorBody};
use crate::params::{QueryParams, parse_query};

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
pub async fn logs(State(api): State<Api>, Query(params): Query<QueryParams>) -> ApiResult<Logs> {
    let query = parse_query(params.q.as_deref(), Signal::Logs)?;
    api.run_range_query(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.queries.logs(&query, r.limit)?),
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
pub async fn log_groups(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<LogGroups> {
    let query = parse_query(params.q.as_deref(), Signal::Logs)?;
    api.run_range_query(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.queries.log_groups(&query, r.limit)?),
    )
    .await
}
