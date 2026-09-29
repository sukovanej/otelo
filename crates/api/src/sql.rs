use axum::Json;
use axum::extract::State;
use otelo_storage::query::SqlResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Api;
use crate::error::{ApiResult, ErrorBody};

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
pub async fn sql(State(api): State<Api>, Json(request): Json<SqlRequest>) -> ApiResult<SqlResult> {
    let sql = request.sql;
    api.run_range_query(
        [request.since, request.until],
        None,
        (request.limit, 100),
        move |r| Ok(r.queries.sql(&sql, r.limit)?),
    )
    .await
}
