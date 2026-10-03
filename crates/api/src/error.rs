use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::QUERY_TIME_LIMIT;

/// The body of every error response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn bad_request(error: &impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: format!("{error:#}"),
        }
    }

    pub const fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }
}

impl From<otelo_indexed_storage::Error> for ApiError {
    fn from(error: otelo_indexed_storage::Error) -> Self {
        match error {
            otelo_indexed_storage::Error::InvalidQuery(message) => Self::bad_request(&message),
            otelo_indexed_storage::Error::TimedOut => Self {
                status: StatusCode::BAD_REQUEST,
                message: format!(
                    "the query ran longer than {} s; narrow the range or the query",
                    QUERY_TIME_LIMIT.as_secs()
                ),
            },
            otelo_indexed_storage::Error::Backend(error) => error.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{error:#}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // In the request's span, this marks the span as failed.
        if self.status.is_server_error() {
            tracing::error!("{}", self.message);
        }
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;
