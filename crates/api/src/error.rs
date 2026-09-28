//! The errors of the API, each a status and a JSON body.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use siner_telemetry::query::InvalidQuery;
use utoipa::ToSchema;

use crate::TIME_LIMIT;

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
