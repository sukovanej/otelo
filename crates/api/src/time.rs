use anyhow::{Context, ensure};
use jiff::{SpanRelativeTo, Timestamp};

use crate::MAX_ROW_LIMIT;

pub fn check_limit(limit: Option<usize>, default_limit: usize) -> anyhow::Result<usize> {
    let limit = limit.unwrap_or(default_limit);
    ensure!(
        (1..=MAX_ROW_LIMIT).contains(&limit),
        "the limit is {limit}, and it has to be from 1 to {MAX_ROW_LIMIT}"
    );
    Ok(limit)
}

#[must_use]
pub fn convert_to_unix_nanos(timestamp: Timestamp) -> i64 {
    i64::try_from(timestamp.as_nanosecond()).unwrap_or(i64::MAX)
}

pub fn parse_time(text: &str, now: i64) -> anyhow::Result<i64> {
    if let Ok(timestamp) = text.parse::<Timestamp>() {
        return Ok(convert_to_unix_nanos(timestamp));
    }
    let length_before_now = parse_duration(text).with_context(|| {
        format!("{text:?} is neither a duration such as 1h nor an RFC 3339 timestamp")
    })?;
    Ok(now.saturating_sub(length_before_now))
}

pub fn parse_duration(text: &str) -> anyhow::Result<i64> {
    let span: jiff::Span = text
        .parse()
        .with_context(|| format!("{text:?} is not a duration such as 500ms, 1h, or 2d"))?;
    let duration = span.to_duration(SpanRelativeTo::days_are_24_hours())?;
    ensure!(!duration.is_negative(), "{text:?} is negative");
    Ok(i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
}
