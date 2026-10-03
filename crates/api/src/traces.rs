use axum::extract::{Path, Query, State};
use otelo_indexed_storage::TraceId;
use otelo_indexed_storage::query::{Spans, Trace, Traces};
use otelo_query::Signal;

use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::params::{LookupParams, QueryParams, parse_query};
use crate::{Api, DefaultSince, RangeSignals, RequestedRange};

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
pub async fn list_spans(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Spans> {
    let query = parse_query(params.query.as_deref(), Signal::Spans)?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Spans),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        100,
        move |opened, limit| Ok(opened.queries.list_spans(&query, limit)?),
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
pub async fn list_traces(
    State(api): State<Api>,
    Query(params): Query<QueryParams>,
) -> ApiResult<Traces> {
    let query = parse_query(params.query.as_deref(), Signal::Spans)?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Spans),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        50,
        move |opened, limit| Ok(opened.queries.list_traces(&query, limit)?),
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
pub async fn get_trace(
    State(api): State<Api>,
    Path(trace_id_hex): Path<String>,
    Query(params): Query<LookupParams>,
) -> ApiResult<Trace> {
    let trace_id =
        TraceId::parse_hex(&trace_id_hex).map_err(|error| ApiError::bad_request(&error))?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::SpansAndLogs,
            since: params.since,
            until: params.until,
            default_since: DefaultSince::OldestRetained,
        },
        params.limit,
        1000,
        move |opened, limit| {
            opened.queries.get_trace(trace_id, limit)?.ok_or_else(|| {
                ApiError::not_found(format!("no spans or logs of trace {trace_id_hex}"))
            })
        },
    )
    .await
}
