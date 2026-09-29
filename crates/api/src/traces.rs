use axum::extract::{Path, Query, State};
use siner_query::Signal;
use siner_storage::TraceId;
use siner_storage::query::{Spans, Trace, Traces};

use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::params::{LookupParams, QueryParams, parse_query};
use crate::{Api, WHOLE_RETENTION};

/// Spans, newest first.
#[utoipa::path(
    get,
    path = "/api/spans",
    params(QueryParams),
    responses(
        (status = 200, body = Spans),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn spans(State(api): State<Api>, Query(params): Query<QueryParams>) -> ApiResult<Spans> {
    let query = parse_query(params.q.as_deref(), Signal::Spans)?;
    api.run_range_query(
        [params.since, params.until],
        None,
        (params.limit, 100),
        move |r| Ok(r.queries.spans(&query, r.limit)?),
    )
    .await
}

/// Traces with a span that the query keeps, by their root span, newest first.
#[utoipa::path(
    get,
    path = "/api/traces",
    params(QueryParams),
    responses(
        (status = 200, body = Traces),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn traces(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Traces> {
    let query = parse_query(params.q.as_deref(), Signal::Spans)?;
    api.run_range_query(
        [params.since, params.until],
        None,
        (params.limit, 50),
        move |r| Ok(r.queries.traces(&query, r.limit)?),
    )
    .await
}

/// One trace: its spans by start time, and the logs that carry its ID.
#[utoipa::path(
    get,
    path = "/api/traces/{trace_id}",
    params(
        ("trace_id" = String, Path, description = "The trace ID as 32 hex digits"),
        LookupParams,
    ),
    responses(
        (status = 200, body = Trace),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub async fn trace(
    State(api): State<Api>,
    Path(trace_id): Path<String>,
    Query(params): Query<LookupParams>,
) -> ApiResult<Trace> {
    let id = TraceId::parse_hex(&trace_id).map_err(|e| ApiError::bad_request(&e))?;
    api.run_range_query(
        [params.since, params.until],
        WHOLE_RETENTION,
        (params.limit, 1000),
        move |r| {
            r.queries
                .trace(id, r.limit)?
                .ok_or_else(|| ApiError::not_found(format!("no spans or logs of trace {trace_id}")))
        },
    )
    .await
}
