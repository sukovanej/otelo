//! The endpoints that describe what a query can read: the attributes, the
//! completion of a query, and the indexed attributes.

use std::fmt;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use siner_query::{Signal, SuggestionKind};
use siner_telemetry::IndexedKey;
use siner_telemetry::query::{Attributes, ReaderCatalog};
use utoipa::{IntoParams, ToSchema};

use super::{Api, ApiError, ApiResult, ErrorBody, WHOLE_RETENTION, parse_signal};

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

/// A parameter of a signal stays a string, so a wrong one gets the JSON of
/// an error from `parse_signal`; the spec still names the values it takes.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct SignalParams {
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
        (status = 200, body = Attributes),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn attributes(
    State(api): State<Api>,
    Query(params): Query<SignalParams>,
) -> ApiResult<Attributes> {
    let signal = parse_signal(&params.signal)?;
    api.run([None, None], WHOLE_RETENTION, (None, 1), move |r| {
        Ok(r.reader.attributes(signal)?)
    })
    .await
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct CompleteParams {
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
pub(super) async fn complete(
    State(api): State<Api>,
    Query(params): Query<CompleteParams>,
) -> ApiResult<Completions> {
    let signal = parse_signal(&params.signal)?;
    let q = params.q.unwrap_or_default();
    let cursor = params.cursor.map_or(q.len(), |chars| {
        q.char_indices().nth(chars).map_or(q.len(), |(i, _)| i)
    });
    api.run([None, None], WHOLE_RETENTION, (None, 1), move |r| {
        let chars = |byte: usize| q[..byte].chars().count();
        let suggestions = siner_query::complete(&q, cursor, signal, &ReaderCatalog(&r.reader))
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

/// The attributes that have an index.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexList {
    pub indexes: Vec<IndexBody>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexBody {
    /// `logs` or `spans`.
    pub signal: SignalName,
    pub key: String,
}

impl Api {
    fn index_list(&self) -> IndexList {
        IndexList {
            indexes: self
                .indexes
                .get()
                .into_iter()
                .map(|key| IndexBody {
                    signal: key.signal.into(),
                    key: key.key,
                })
                .collect(),
        }
    }

    /// Stores a change to the indexed attributes and hands the new set to the
    /// writer.
    async fn change_index(&self, signal: &str, key: &str, add: bool) -> ApiResult<IndexList> {
        let key =
            IndexedKey::new(parse_signal(signal)?, key).map_err(|e| ApiError::bad_request(&e))?;
        let api = self.clone();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || -> Result<IndexList, ApiError> {
            let _entered = span.enter();
            if add {
                api.state.add_index(&key)?;
            } else if !api.state.remove_index(&key)? {
                return Err(ApiError::not_found(format!(
                    "{} {} has no index",
                    key.signal, key.key
                )));
            }
            api.indexes.set(api.state.indexes()?);
            Ok(api.index_list())
        })
        .await
        .map_err(|e| ApiError::from(anyhow::Error::from(e)))?
        .map(Json)
    }
}

/// The attributes that have an index.
#[utoipa::path(
    get,
    path = "/api/indexes",
    responses((status = 200, body = IndexList)),
)]
pub(super) async fn list_indexes(State(api): State<Api>) -> Json<IndexList> {
    Json(api.index_list())
}

/// Indexes an attribute of the logs or of the spans in every day file, so a
/// query that compares it reads only the matching records. The writer builds
/// the index within seconds.
#[utoipa::path(
    put,
    path = "/api/indexes/{signal}/{key}",
    params(
        ("signal" = SignalName, Path, description = "`logs` or `spans`"),
        ("key" = String, Path, description = "The attribute key, such as `user.id`"),
    ),
    responses(
        (status = 200, body = IndexList),
        (status = 400, body = ErrorBody),
    ),
)]
pub(super) async fn add_index(
    State(api): State<Api>,
    Path((signal, key)): Path<(String, String)>,
) -> ApiResult<IndexList> {
    api.change_index(&signal, &key, true).await
}

/// Drops the index of an attribute from every day file.
#[utoipa::path(
    delete,
    path = "/api/indexes/{signal}/{key}",
    params(
        ("signal" = SignalName, Path, description = "`logs` or `spans`"),
        ("key" = String, Path, description = "The attribute key, such as `user.id`"),
    ),
    responses(
        (status = 200, body = IndexList),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub(super) async fn remove_index(
    State(api): State<Api>,
    Path((signal, key)): Path<(String, String)>,
) -> ApiResult<IndexList> {
    api.change_index(&signal, &key, false).await
}
