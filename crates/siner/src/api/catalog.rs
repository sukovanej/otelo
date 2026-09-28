//! The endpoints that describe what a query can read: the attributes, the
//! completion of a query, and the indexed attributes.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use siner_telemetry::IndexedKey;
use siner_telemetry::query::{Attributes, ReaderCatalog};
use utoipa::{IntoParams, ToSchema};

use super::{Api, ApiError, ApiResult, ErrorBody, WHOLE_RETENTION, parse_signal};

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(super) struct SignalParams {
    /// `logs`, `spans`, or `metrics`.
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
    /// `logs`, `spans`, or `metrics`.
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
    /// `field`, `operator`, `value`, or `keyword`.
    pub kind: String,
    /// The type of a field and how many records have it, or how many records
    /// have a value.
    pub detail: Option<String>,
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
                kind: s.kind.as_str().into(),
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
    pub signal: String,
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
                    signal: key.signal.as_str().into(),
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
        ("signal" = String, Path, description = "`logs` or `spans`"),
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
        ("signal" = String, Path, description = "`logs` or `spans`"),
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
