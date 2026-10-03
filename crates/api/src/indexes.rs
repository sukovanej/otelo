use std::fmt;

use axum::Json;
use axum::extract::{Path, State};
use otelo_indexed_storage::{IndexedAttribute, IndexedSignal};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Api;
use crate::error::{ApiError, ApiResult, ErrorBody};
use crate::params::parse_signal;

/// The attributes that have an index.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexList {
    pub indexes: Vec<IndexBody>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexBody {
    /// `logs` or `spans`.
    pub signal: IndexedSignalName,
    pub key: String,
}

/// The kind of record whose attributes can have an index, as the API names
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = IndexedSignal)]
pub enum IndexedSignalName {
    Logs,
    Spans,
}

impl From<IndexedSignal> for IndexedSignalName {
    fn from(signal: IndexedSignal) -> Self {
        match signal {
            IndexedSignal::Logs => Self::Logs,
            IndexedSignal::Spans => Self::Spans,
        }
    }
}

impl fmt::Display for IndexedSignalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let signal = match self {
            Self::Logs => IndexedSignal::Logs,
            Self::Spans => IndexedSignal::Spans,
        };
        signal.fmt(f)
    }
}

#[derive(Clone, Copy)]
enum IndexChange {
    Add,
    Remove,
}

impl Api {
    fn list_indexed_attributes(&self) -> IndexList {
        IndexList {
            indexes: self
                .storage
                .indexed_attributes()
                .into_iter()
                .map(|attribute| IndexBody {
                    signal: attribute.signal().into(),
                    key: attribute.key().to_owned(),
                })
                .collect(),
        }
    }

    async fn change_index(
        &self,
        signal: &str,
        key: &str,
        change: IndexChange,
    ) -> ApiResult<IndexList> {
        let attribute = IndexedSignal::try_from(parse_signal(signal)?)
            .and_then(|signal| IndexedAttribute::new(signal, key))
            .map_err(|error| ApiError::bad_request(&error))?;
        let api = self.clone();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || -> Result<IndexList, ApiError> {
            let _entered = span.enter();
            match change {
                IndexChange::Add => api.storage.add_index(&attribute)?,
                IndexChange::Remove => {
                    if !api.storage.remove_index(&attribute)? {
                        return Err(ApiError::not_found(format!(
                            "{} {} has no index",
                            attribute.signal(),
                            attribute.key()
                        )));
                    }
                }
            }
            Ok(api.list_indexed_attributes())
        })
        .await
        .map_err(|error| ApiError::from(anyhow::Error::from(error)))?
        .map(Json)
    }
}

/// The attributes that have an index.
#[utoipa::path(
    get,
    path = "/api/indexes",
    responses((status = 200, body = IndexList)),
)]
pub async fn list_indexes(State(api): State<Api>) -> Json<IndexList> {
    Json(api.list_indexed_attributes())
}

/// Indexes an attribute of the logs or of the spans, so a query that
/// compares it reads only the matching records. The writer builds
/// the index within seconds.
#[utoipa::path(
    put,
    path = "/api/indexes/{signal}/{key}",
    params(
        ("signal" = IndexedSignalName, Path, description = "`logs` or `spans`"),
        ("key" = String, Path, description = "The attribute key, such as `user.id`"),
    ),
    responses(
        (status = 200, body = IndexList),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn add_index(
    State(api): State<Api>,
    Path((signal, key)): Path<(String, String)>,
) -> ApiResult<IndexList> {
    api.change_index(&signal, &key, IndexChange::Add).await
}

/// Drops the index of an attribute.
#[utoipa::path(
    delete,
    path = "/api/indexes/{signal}/{key}",
    params(
        ("signal" = IndexedSignalName, Path, description = "`logs` or `spans`"),
        ("key" = String, Path, description = "The attribute key, such as `user.id`"),
    ),
    responses(
        (status = 200, body = IndexList),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub async fn remove_index(
    State(api): State<Api>,
    Path((signal, key)): Path<(String, String)>,
) -> ApiResult<IndexList> {
    api.change_index(&signal, &key, IndexChange::Remove).await
}
