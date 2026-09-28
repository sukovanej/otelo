//! The times, durations, and limits that the endpoints take.

use anyhow::{Context, ensure};
use jiff::{SpanRelativeTo, Timestamp};

use crate::MAX_LIMIT;

pub fn check_limit(limit: Option<usize>, default: usize) -> anyhow::Result<usize> {
    let limit = limit.unwrap_or(default);
    ensure!(
        (1..=MAX_LIMIT).contains(&limit),
        "the limit is {limit}, and it has to be from 1 to {MAX_LIMIT}"
    );
    Ok(limit)
}

/// A timestamp in unix nanoseconds, capped at the largest `i64`.
#[must_use]
pub fn nanos(ts: Timestamp) -> i64 {
    i64::try_from(ts.as_nanosecond()).unwrap_or(i64::MAX)
}

/// A duration before `now`, such as `1h`, or an RFC 3339 timestamp.
///
/// # Errors
///
/// When `text` is neither.
pub fn parse_time(text: &str, now: i64) -> anyhow::Result<i64> {
    if let Ok(ts) = text.parse::<Timestamp>() {
        return Ok(nanos(ts));
    }
    let ago = parse_duration(text).with_context(|| {
        format!("{text:?} is neither a duration such as 1h nor an RFC 3339 timestamp")
    })?;
    Ok(now.saturating_sub(ago))
}

/// A duration such as `500ms`, `1h`, or `2d`, in nanoseconds.
///
/// # Errors
///
/// When `text` is not a duration, or is negative.
pub fn parse_duration(text: &str) -> anyhow::Result<i64> {
    let span: jiff::Span = text
        .parse()
        .with_context(|| format!("{text:?} is not a duration such as 500ms, 1h, or 2d"))?;
    let duration = span.to_duration(SpanRelativeTo::days_are_24_hours())?;
    ensure!(!duration.is_negative(), "{text:?} is negative");
    Ok(i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
}
