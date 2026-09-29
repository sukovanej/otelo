use serde::Deserialize;
use siner_query::Signal;
use siner_storage::query;
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

pub fn parse_step_ns(text: Option<&str>) -> Result<Option<i64>, ApiError> {
    text.map(parse_duration)
        .transpose()
        .map_err(|e| ApiError::bad_request(&e))
}

pub fn resolve_step(
    request: &Request,
    requested_step_ns: Option<i64>,
    default_max_buckets: i64,
) -> Result<i64, ApiError> {
    let step_ns =
        requested_step_ns.unwrap_or_else(|| query::step_for(request.range, default_max_buckets));
    if step_ns <= 0 || request.range.length() / step_ns > query::MAX_BUCKETS {
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

pub fn parse_query(q: Option<&str>, signal: Signal) -> Result<siner_query::Query, ApiError> {
    siner_query::parse(q.unwrap_or_default(), signal).map_err(|e| ApiError::bad_request(&e))
}
