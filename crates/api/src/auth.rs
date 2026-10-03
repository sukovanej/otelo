use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use axum::Extension;
use axum::Json;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, COOKIE, HOST, ORIGIN, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use otelo_state::{SessionToken, StateFile};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;
use utoipa::ToSchema;

use crate::Api;
use crate::error::{ApiError, ErrorBody};

pub const SESSION_COOKIE_NAME: &str = "otelo_session";

// Browsers keep a cookie for 400 days at most, and the daemon ends a session 30 days after
// its last use.
const SESSION_COOKIE_MAX_AGE_SECONDS: u32 = 400 * 24 * 3600;

// argon2id takes a CPU and about 19 MiB per check, and the droplet has one CPU.
const LOGIN_CHECK_INTERVAL: Duration = Duration::from_secs(1);

const LOGIN_PATH: &str = "/api/login";

/// The password that `otelo init` printed.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginBody {
    pub password: String,
}

pub struct LoginGate {
    next_check_at: Mutex<Instant>,
}

impl LoginGate {
    pub fn new() -> Self {
        Self {
            next_check_at: Mutex::new(Instant::now()),
        }
    }

    async fn check_password(
        &self,
        state: Arc<StateFile>,
        candidate: String,
    ) -> anyhow::Result<bool> {
        let mut next_check_at = self.next_check_at.lock().await;
        tokio::time::sleep_until(*next_check_at).await;
        let started_at = Instant::now();
        *next_check_at = started_at + LOGIN_CHECK_INTERVAL;
        let is_password = tokio::task::spawn_blocking(move || state.is_password(&candidate))
            .await
            .context("check the password")??;
        drop(next_check_at);
        if !is_password {
            tokio::time::sleep_until(started_at + LOGIN_CHECK_INTERVAL).await;
        }
        Ok(is_password)
    }
}

/// Trades the password for a session, which the answer sets as the cookie
/// `otelo_session`. The daemon checks one password a second, and a wrong
/// one answers after that second.
#[utoipa::path(
    post,
    path = "/api/login",
    request_body = LoginBody,
    responses(
        (status = 204, description = "The session is in the cookie `otelo_session`"),
        (status = 401, body = ErrorBody),
    ),
)]
pub async fn log_in(
    State(api): State<Api>,
    headers: HeaderMap,
    Json(login): Json<LoginBody>,
) -> Result<Response, ApiError> {
    if !api
        .login_gate
        .check_password(Arc::clone(&api.state), login.password)
        .await?
    {
        return Err(ApiError::unauthorized("the password is wrong".into()));
    }
    let state = Arc::clone(&api.state);
    let token = tokio::task::spawn_blocking(move || state.start_session())
        .await
        .context("start a session")??;
    Ok((
        StatusCode::NO_CONTENT,
        [(SET_COOKIE, build_session_cookie(&token, &headers))],
    )
        .into_response())
}

/// Ends the session of the request.
#[utoipa::path(
    post,
    path = "/api/logout",
    responses((status = 204, description = "The session has ended")),
)]
pub async fn log_out(
    State(api): State<Api>,
    Extension(token): Extension<SessionToken>,
) -> Result<Response, ApiError> {
    let state = Arc::clone(&api.state);
    tokio::task::spawn_blocking(move || state.end_session(&token))
        .await
        .context("end the session")??;
    let expired_cookie =
        format!("{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    Ok((
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            HeaderValue::from_str(&expired_cookie).expect("the cookie is ASCII"),
        )],
    )
        .into_response())
}

pub async fn require_session(State(api): State<Api>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if path != "/api" && !path.starts_with("/api/") {
        return next.run(request).await;
    }
    let is_write = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if is_write && !is_from_own_origin(request.headers()) {
        return ApiError::forbidden("the request comes from another origin".into()).into_response();
    }
    if request.method() == Method::POST && path == LOGIN_PATH {
        return next.run(request).await;
    }
    let Some(token) = read_session_token(request.headers()) else {
        return ApiError::unauthorized(
            "no session; log in with the password of `otelo init`".into(),
        )
        .into_response();
    };
    let state = Arc::clone(&api.state);
    let renewal_token = token.clone();
    let renewed = tokio::task::spawn_blocking(move || state.renew_session(&renewal_token))
        .await
        .context("renew the session");
    match renewed {
        Ok(Ok(true)) => {}
        Ok(Ok(false)) => {
            return ApiError::unauthorized("the session has ended; log in again".into())
                .into_response();
        }
        Ok(Err(error)) | Err(error) => return ApiError::from(error).into_response(),
    }
    request.extensions_mut().insert(token);
    next.run(request).await
}

fn read_session_token(headers: &HeaderMap) -> Option<SessionToken> {
    let bearer_token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok()?.strip_prefix("Bearer "));
    let cookie_token = || {
        headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|cookies| cookies.split(';'))
            .find_map(|cookie| {
                cookie
                    .trim()
                    .strip_prefix(SESSION_COOKIE_NAME)?
                    .strip_prefix('=')
            })
    };
    bearer_token.or_else(cookie_token)?.trim().parse().ok()
}

// Browsers send Origin with every write, and the CLI sends none.
fn is_from_own_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let origin_authority = origin
        .to_str()
        .ok()
        .and_then(|origin| origin.split_once("://"))
        .map(|(_, authority)| authority);
    origin_authority.is_some_and(|authority| {
        headers
            .get(HOST)
            .is_some_and(|host| host.as_bytes() == authority.as_bytes())
    })
}

fn build_session_cookie(token: &SessionToken, headers: &HeaderMap) -> HeaderValue {
    let secure = if is_loopback_host(headers) {
        ""
    } else {
        "; Secure"
    };
    let cookie = format!(
        "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; \
         Max-Age={SESSION_COOKIE_MAX_AGE_SECONDS}{secure}"
    );
    HeaderValue::from_str(&cookie).expect("the cookie is ASCII")
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
