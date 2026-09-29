use std::fmt;

use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use siner_query::{Signal, SuggestionKind};
use siner_storage::query::AttributeKeys;
use utoipa::{IntoParams, ToSchema};

use crate::error::{ApiResult, ErrorBody};
use crate::params::parse_signal;
use crate::{Api, WHOLE_RETENTION};

/// The kind of record a query reads, as the API names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = Signal)]
pub enum SignalName {
    Logs,
    Spans,
    Metrics,
}

impl From<Signal> for SignalName {
    fn from(signal: Signal) -> Self {
        match signal {
            Signal::Logs => Self::Logs,
            Signal::Spans => Self::Spans,
            Signal::Metrics => Self::Metrics,
        }
    }
}

impl fmt::Display for SignalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signal = match self {
            Self::Logs => Signal::Logs,
            Self::Spans => Signal::Spans,
            Self::Metrics => Signal::Metrics,
        };
        f.write_str(signal.as_str())
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SignalParams {
    // A string, so a wrong signal gets the JSON error of `parse_signal`.
    #[param(value_type = SignalName)]
    signal: String,
}

/// The attribute keys of the records of a signal and of their resources over
/// the retention, the most common first, with their types.
#[utoipa::path(
    get,
    path = "/api/attributes",
    params(SignalParams),
    responses(
        (status = 200, body = AttributeKeys),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn attributes(
    State(api): State<Api>,
    Query(params): Query<SignalParams>,
) -> ApiResult<AttributeKeys> {
    let signal = parse_signal(&params.signal)?;
    api.run_range_query([None, None], WHOLE_RETENTION, (None, 1), move |r| {
        Ok(r.queries.attributes(signal)?)
    })
    .await
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CompleteParams {
    #[param(value_type = SignalName)]
    signal: String,
    /// The query as typed so far.
    q: Option<String>,
    /// The position of the cursor in the query, in characters. The end of the
    /// query when missing.
    cursor: Option<usize>,
}

/// What can go at the cursor of a query.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Completions {
    pub suggestions: Vec<SuggestionBody>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SuggestionBody {
    /// The text to put in place of the characters from `start` to `end`.
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub kind: CompletionKind,
    /// The type of a field and how many records have it, or how many records
    /// have a value.
    #[schema(required = true)]
    pub detail: Option<String>,
}

/// What the text of a suggestion is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CompletionKind {
    Field,
    Operator,
    Value,
    Keyword,
}

impl From<SuggestionKind> for CompletionKind {
    fn from(kind: SuggestionKind) -> Self {
        match kind {
            SuggestionKind::Field => Self::Field,
            SuggestionKind::Operator => Self::Operator,
            SuggestionKind::Value => Self::Value,
            SuggestionKind::Keyword => Self::Keyword,
        }
    }
}

impl fmt::Display for CompletionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Field => SuggestionKind::Field,
            Self::Operator => SuggestionKind::Operator,
            Self::Value => SuggestionKind::Value,
            Self::Keyword => SuggestionKind::Keyword,
        };
        f.write_str(kind.as_str())
    }
}

/// Suggests the fields, operators, values, and keywords that can go at the
/// cursor of a query, from the attributes and values of the retention.
#[utoipa::path(
    get,
    path = "/api/complete",
    params(CompleteParams),
    responses(
        (status = 200, body = Completions),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn complete(
    State(api): State<Api>,
    Query(params): Query<CompleteParams>,
) -> ApiResult<Completions> {
    let signal = parse_signal(&params.signal)?;
    let q = params.q.unwrap_or_default();
    let cursor = params.cursor.map_or(q.len(), |chars| {
        q.char_indices().nth(chars).map_or(q.len(), |(i, _)| i)
    });
    api.run_range_query([None, None], WHOLE_RETENTION, (None, 1), move |r| {
        let chars = |byte: usize| q[..byte].chars().count();
        let suggestions = siner_query::complete(&q, cursor, signal, &*r.queries)
            .into_iter()
            .map(|s| SuggestionBody {
                start: chars(s.replace.start),
                end: chars(s.replace.end),
                text: s.text,
                kind: s.kind.into(),
                detail: s.detail,
            })
            .collect();
        Ok(Completions { suggestions })
    })
    .await
}
