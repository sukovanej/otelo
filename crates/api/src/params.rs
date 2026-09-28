//! The parameters that several endpoints share, and how they are read.

use serde::Deserialize;
use siner_query::Signal;
use siner_telemetry::query;
use utoipa::IntoParams;

use crate::Request;
use crate::error::ApiError;
use crate::time::parse_duration;

/// The range, the limit, and the query of a list.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct QueryParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. One hour before `until` when missing.
    pub since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    pub until: Option<String>,
    /// The most rows to return.
    pub limit: Option<usize>,
    /// The records to keep, such as `http.route = "/matches" OR user.id = 7`.
    /// Every record when missing.
    pub q: Option<String>,
}

/// The range and the limit of a lookup.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LookupParams {
    /// The start of the range: a duration before now, such as `1h`, or an
    /// RFC 3339 timestamp. The whole retention when missing.
    pub since: Option<String>,
    /// The end of the range, in the form of `since`. Now when missing.
    pub until: Option<String>,
    /// The most spans, and the most logs, to return.
    pub limit: Option<usize>,
}

/// A step in nanoseconds, or `None` when `text` is missing.
pub fn parse_step(text: Option<&str>) -> Result<Option<i64>, ApiError> {
    text.map(parse_duration)
        .transpose()
        .map_err(|e| ApiError::bad_request(&e))
}

/// The step, or the smallest round one that makes `buckets` buckets at most
/// in the range, when it makes no more than [`query::MAX_BUCKETS`].
pub fn check_step(r: &Request, step: Option<i64>, buckets: i64) -> Result<i64, ApiError> {
    let (since, until) = (r.reader.since(), r.reader.until());
    let step_ns = step.unwrap_or_else(|| query::step_for(since, until, buckets));
    if step_ns <= 0 || (until - since) / step_ns > query::MAX_BUCKETS {
        return Err(ApiError::bad_request(&format!(
            "the step makes more than {} buckets in the range; raise the step",
            query::MAX_BUCKETS
        )));
    }
    Ok(step_ns)
}

pub fn parse_signal(text: &str) -> Result<Signal, ApiError> {
    text.parse().map_err(|e: String| ApiError::bad_request(&e))
}

/// Parses the query `q` over `signal`. A missing query keeps every record.
pub fn parse_query(q: Option<&str>, signal: Signal) -> Result<siner_query::Query, ApiError> {
    siner_query::parse(q.unwrap_or_default(), signal).map_err(|e| ApiError::bad_request(&e))
}
