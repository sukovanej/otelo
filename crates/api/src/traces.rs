use axum::extract::{Path, Query, State};
use otelo_indexed_storage::TraceId;
use otelo_indexed_storage::query::{SpanGroupingField, SpanGroups, Spans, Trace, Traces};
use otelo_query::Signal;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::params::{
    LookupParams, QueryParams, parse_field_list, parse_query, parse_step_ns, resolve_step,
};
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

/// The range, the limit, the query, the grouping, and the step of span groups.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SpanGroupParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    until: Option<String>,
    /// The most groups to return.
    limit: Option<usize>,
    /// The spans to keep, such as `service = "api" kind = server`. Every span
    /// when missing.
    #[serde(rename = "q")]
    #[param(rename = "q")]
    query: Option<String>,
    /// The names to group the spans by, separated by commas: attributes,
    /// `service`, `name`, or `resource.<key>`, such as
    /// `http.request.method,http.route`. All spans are one group when
    /// missing.
    by: Option<String>,
    /// The length of a bucket, such as `1m`. One that makes 120 buckets at
    /// most when missing.
    step: Option<String>,
}

/// The spans that the query keeps, grouped by the values of the `by` names,
/// the most time first. Each group has the count, the failures, the total
/// time, and the latency percentiles of its spans, and the name and the
/// attributes of its newest span. The answer has the same numbers for all the
/// spans, over the range and in buckets of one step.
#[utoipa::path(
    get,
    path = "/api/spans/groups",
    params(SpanGroupParams),
    responses(
        (status = 200, body = SpanGroups),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn list_span_groups(
    State(api): State<Api>,
    Query(params): Query<SpanGroupParams>,
) -> ApiResult<SpanGroups> {
    let query = parse_query(params.query.as_deref(), Signal::Spans)?;
    let by: Vec<SpanGroupingField> = parse_field_list(params.by.as_deref())?;
    let requested_step_ns = parse_step_ns(params.step.as_deref())?;
    api.run_limited_range_query(
        RequestedRange {
            signals: RangeSignals::One(Signal::Spans),
            since: params.since,
            until: params.until,
            default_since: DefaultSince::HourBeforeNow,
        },
        params.limit,
        50,
        move |opened, limit| {
            let step_ns = resolve_step(opened.range, requested_step_ns, 120)?;
            Ok(opened
                .queries
                .list_span_groups(&query, &by, step_ns, limit)?)
        },
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
