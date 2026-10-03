use std::sync::Arc;

use anyhow::Context;
use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, COOKIE, HOST, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Api;
use crate::error::{ApiError, ErrorBody};

const PASSWORD_COOKIE_NAME: &str = "otelo_password";

// Browsers keep a cookie for 400 days at most.
const PASSWORD_COOKIE_MAX_AGE_SECONDS: u32 = 400 * 24 * 3600;

const OPEN_PATHS: [&str; 2] = ["/api/login", "/api/logout"];

/// The password that `otelo init` printed.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginBody {
    pub password: String,
}

/// Checks the password, and sets it as the cookie `otelo_password`, which
/// the browser then sends with every request.
#[utoipa::path(
    post,
    path = "/api/login",
    request_body = LoginBody,
    responses(
        (status = 204, description = "The password is in the cookie `otelo_password`"),
        (status = 401, body = ErrorBody),
    ),
)]
pub async fn log_in(
    State(api): State<Api>,
    headers: HeaderMap,
    Json(login): Json<LoginBody>,
) -> Result<Response, ApiError> {
    if !api.is_password(login.password.clone()).await? {
        return Err(ApiError::unauthorized("the password is wrong".into()));
    }
    let secure = if is_loopback_host(&headers) {
        ""
    } else {
        "; Secure"
    };
    let cookie = format!(
        "{PASSWORD_COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Strict; \
         Max-Age={PASSWORD_COOKIE_MAX_AGE_SECONDS}{secure}",
        login.password
    );
    let cookie = HeaderValue::from_str(&cookie).context("set the password as a cookie")?;
    Ok((StatusCode::NO_CONTENT, [(SET_COOKIE, cookie)]).into_response())
}

/// Clears the cookie `otelo_password`.
#[utoipa::path(
    post,
    path = "/api/logout",
    responses((status = 204, description = "The cookie is cleared")),
)]
pub async fn log_out() -> Response {
    let expired_cookie = HeaderValue::from_str(&format!(
        "{PASSWORD_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
    ))
    .expect("the cookie is ASCII");
    (StatusCode::NO_CONTENT, [(SET_COOKIE, expired_cookie)]).into_response()
}

pub async fn require_password(State(api): State<Api>, request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let is_api_path = path == "/api" || path.starts_with("/api/");
    if !is_api_path || (request.method() == Method::POST && OPEN_PATHS.contains(&path)) {
        return next.run(request).await;
    }
    let Some(password) = read_password(request.headers()) else {
        return ApiError::unauthorized("no password; send the password of `otelo init`".into())
            .into_response();
    };
    match api.is_password(password.to_owned()).await {
        Ok(true) => next.run(request).await,
        Ok(false) => ApiError::unauthorized("the password is wrong".into()).into_response(),
        Err(error) => error.into_response(),
    }
}

impl Api {
    async fn is_password(&self, candidate: String) -> Result<bool, ApiError> {
        let state = Arc::clone(&self.state);
        Ok(
            tokio::task::spawn_blocking(move || state.is_password(&candidate))
                .await
                .context("check the password")??,
        )
    }
}

fn read_password(headers: &HeaderMap) -> Option<&str> {
    let bearer_password = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok()?.strip_prefix("Bearer "));
    let cookie_password = || {
        headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|cookies| cookies.split(';'))
            .find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix(PASSWORD_COOKIE_NAME)?
                    .strip_prefix('=')
            })
    };
    bearer_password.or_else(cookie_password).map(str::trim)
}

fn is_loopback_host(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|host| host.to_str().ok()) else {
        return false;
    };
    let host_name = host
        .rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|byte| byte.is_ascii_digit()))
        .map_or(host, |(name, _)| name);
    matches!(host_name, "localhost" | "127.0.0.1" | "[::1]")
}
