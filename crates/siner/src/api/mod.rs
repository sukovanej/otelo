//! The HTTP query API over the telemetry, and a spec of it for client
//! generators at `/api/openapi.json`.
//!
//! Every query endpoint takes a time range, `since` and `until`, a `limit`,
//! and a query `q` in the language of [`siner_query`], and says when it cut
//! the result. A time is a duration before now (`1h`, `30m`, `2d`) or an RFC
//! 3339 timestamp. The range is capped at the retention.

mod catalog;
mod queries;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, ensure};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use jiff::{SpanRelativeTo, Timestamp};
use serde::{Deserialize, Serialize};
use siner_query::Signal;
use siner_telemetry::query::InvalidQuery;
use siner_telemetry::{Day, Indexes, Reader};
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

pub use catalog::{Completions, IndexList};
pub use queries::SqlRequest;

use crate::state::State;

/// The most rows any query returns.
const MAX_LIMIT: usize = 10_000;

/// How long one query can run.
const TIME_LIMIT: Duration = Duration::from_secs(10);

/// The range of a query that names no `since`.
const DEFAULT_RANGE: &str = "1h";

#[derive(OpenApi)]
#[openapi(info(
    title = "siner",
    description = "Query the logs, traces, and metrics that siner keeps."
))]
struct Spec;

/// Where the day files are, how many days of them there are, and which
/// attributes have an index.
#[derive(Clone)]
pub struct Api {
    pub dir: PathBuf,
    pub retention_days: u16,
    pub indexes: Indexes,
    pub state: Arc<State>,
}

/// The query routes and `/api/openapi.json`.
pub fn router(api: Api) -> Router {
    let (router, spec) = OpenApiRouter::with_openapi(Spec::openapi())
        .routes(routes!(queries::logs))
        .routes(routes!(queries::log_groups))
        .routes(routes!(queries::spans))
        .routes(routes!(queries::traces))
        .routes(routes!(queries::trace))
        .routes(routes!(queries::metrics))
        .routes(routes!(queries::metric))
        .routes(routes!(queries::sql))
        .routes(routes!(catalog::attributes))
        .routes(routes!(catalog::complete))
        .routes(routes!(catalog::list_indexes))
        .routes(routes!(catalog::add_index, catalog::remove_index))
        .with_state(api)
        .split_for_parts();
    router.route(
        "/api/openapi.json",
        get(move || async move { Json(spec.clone()) }),
    )
}

/// The body of every error response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(error: &impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: format!("{error:#}"),
        }
    }

    const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        if siner_telemetry::timed_out(&error) {
            return Self {
                status: StatusCode::BAD_REQUEST,
                message: format!(
                    "the query ran longer than {} s; narrow the range or the query",
                    TIME_LIMIT.as_secs()
                ),
            };
        }
        if let Some(invalid) = error.downcast_ref::<InvalidQuery>() {
            return Self::bad_request(invalid);
        }
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{error:#}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

/// The reader and the limit a handler runs its query with.
struct Request {
    reader: Reader,
    limit: usize,
}

/// The range of the whole retention, for queries that look a thing up.
const WHOLE_RETENTION: Option<i64> = Some(i64::MIN);

impl Api {
    /// Checks the range and the limit, opens a reader, and runs `query` with
    /// it on a blocking thread.
    async fn run<T: Send + 'static>(
        &self,
        range: [Option<String>; 2],
        default_since: Option<i64>,
        limit: (Option<usize>, usize),
        query: impl FnOnce(Request) -> Result<T, ApiError> + Send + 'static,
    ) -> ApiResult<T> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let [since, until] = range;
            let (since, until) = api
                .range(since.as_deref(), until.as_deref(), default_since)
                .map_err(|e| ApiError::bad_request(&e))?;
            let limit = check_limit(limit.0, limit.1).map_err(|e| ApiError::bad_request(&e))?;
            let mut reader = Reader::open(&api.dir, since, until)?;
            reader.set_time_limit(TIME_LIMIT)?;
            reader.set_indexes(api.indexes.get());
            query(Request { reader, limit })
        })
        .await
        .context("run the query")?
        .map(Json)
    }

    /// The range in unix nanoseconds, with `since` moved up to the oldest
    /// day the retention keeps.
    fn range(
        &self,
        since: Option<&str>,
        until: Option<&str>,
        default_since: Option<i64>,
    ) -> anyhow::Result<(i64, i64)> {
        let now = nanos(Timestamp::now());
        let until = until.map_or(Ok(now), |text| parse_time(text, now))?;
        let oldest = Day::today()
            .plus(1 - i64::from(self.retention_days.max(1)))
            .start();
        let since = match since {
            Some(text) => parse_time(text, now)?,
            None => default_since.unwrap_or(now - parse_duration(DEFAULT_RANGE)?),
        };
        ensure!(since < until, "since has to be before until");
        Ok((since.max(oldest), until))
    }
}

fn check_limit(limit: Option<usize>, default: usize) -> anyhow::Result<usize> {
    let limit = limit.unwrap_or(default);
    ensure!(
        (1..=MAX_LIMIT).contains(&limit),
        "the limit is {limit}, and it has to be from 1 to {MAX_LIMIT}"
    );
    Ok(limit)
}

fn nanos(ts: Timestamp) -> i64 {
    i64::try_from(ts.as_nanosecond()).unwrap_or(i64::MAX)
}

/// A duration before `now`, such as `1h`, or an RFC 3339 timestamp.
fn parse_time(text: &str, now: i64) -> anyhow::Result<i64> {
    if let Ok(ts) = text.parse::<Timestamp>() {
        return Ok(nanos(ts));
    }
    let ago = parse_duration(text).with_context(|| {
        format!("{text:?} is neither a duration such as 1h nor an RFC 3339 timestamp")
    })?;
    Ok(now.saturating_sub(ago))
}

/// A duration such as `500ms`, `1h`, or `2d`, in nanoseconds.
fn parse_duration(text: &str) -> anyhow::Result<i64> {
    let span: jiff::Span = text
        .parse()
        .with_context(|| format!("{text:?} is not a duration such as 500ms, 1h, or 2d"))?;
    let duration = span.to_duration(SpanRelativeTo::days_are_24_hours())?;
    ensure!(!duration.is_negative(), "{text:?} is negative");
    Ok(i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX))
}

fn parse_signal(text: &str) -> Result<Signal, ApiError> {
    text.parse().map_err(|e: String| ApiError::bad_request(&e))
}

/// Parses the query `q` over `signal`. A missing query keeps every record.
fn parse_query(q: Option<&str>, signal: Signal) -> Result<siner_query::Query, ApiError> {
    siner_query::parse(q.unwrap_or_default(), signal).map_err(|e| ApiError::bad_request(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600 * 1_000_000_000;

    #[test]
    fn parses_durations_and_timestamps() {
        let now = nanos(Timestamp::now());
        assert_eq!(parse_time("1h", now).unwrap(), now - HOUR);
        assert_eq!(parse_time("2d", now).unwrap(), now - 48 * HOUR);
        assert_eq!(parse_duration("500ms").unwrap(), 500_000_000);
        assert_eq!(
            parse_time("2026-09-28T00:00:00Z", now).unwrap(),
            nanos("2026-09-28T00:00:00Z".parse().unwrap())
        );
        assert!(parse_time("yesterday", now).is_err());
    }

    #[test]
    fn caps_the_range_at_the_retention() {
        let dir = tempfile::tempdir().unwrap();
        let api = Api {
            dir: PathBuf::new(),
            retention_days: 7,
            indexes: Indexes::default(),
            state: Arc::new(State::open(dir.path()).unwrap()),
        };
        let (since, _) = api.range(Some("30d"), None, None).unwrap();
        assert_eq!(since, Day::today().plus(-6).start());
        assert!(api.range(Some("1h"), Some("2h"), None).is_err());
    }
}
