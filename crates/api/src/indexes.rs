//! The endpoints of the indexed attributes: which have an index, and adding
//! and dropping one.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use siner_telemetry::IndexedKey;
use utoipa::ToSchema;

use crate::Api;
use crate::catalog::SignalName;
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
pub async fn list_indexes(State(api): State<Api>) -> Json<IndexList> {
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
pub async fn add_index(
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
pub async fn remove_index(
    State(api): State<Api>,
    Path((signal, key)): Path<(String, String)>,
) -> ApiResult<IndexList> {
    api.change_index(&signal, &key, false).await
}
