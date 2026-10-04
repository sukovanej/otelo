use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use otelo_state::{Dashboard, DashboardDefinition, DashboardId, DashboardList};

use crate::Api;
use crate::error::{ApiError, ApiResult, ErrorBody};

fn parse_dashboard_id(text: &str) -> Result<DashboardId, ApiError> {
    text.parse().map_err(|error| ApiError::bad_request(&error))
}

// A definition that breaks a rule of the grid or of a query answers with the
// rule, not with the plain text axum answers on its own.
fn read_dashboard_definition(
    payload: Result<Json<DashboardDefinition>, JsonRejection>,
) -> Result<DashboardDefinition, ApiError> {
    payload
        .map(|Json(definition)| definition)
        .map_err(|rejection| ApiError::bad_request(&rejection.body_text()))
}

fn report_missing_dashboard(id: DashboardId) -> ApiError {
    ApiError::not_found(format!("no dashboard has the ID {id}"))
}

/// The saved dashboards, the most recently changed first.
#[utoipa::path(
    get,
    path = "/api/dashboards",
    responses((status = 200, body = DashboardList)),
)]
pub async fn list_dashboards(State(api): State<Api>) -> ApiResult<DashboardList> {
    api.run_blocking_query(|api| Ok(api.state.list_dashboards()?))
        .await
}

/// Saves a new dashboard.
#[utoipa::path(
    post,
    path = "/api/dashboards",
    request_body = DashboardDefinition,
    responses(
        (status = 200, body = Dashboard),
        (status = 400, body = ErrorBody),
    ),
)]
pub async fn create_dashboard(
    State(api): State<Api>,
    payload: Result<Json<DashboardDefinition>, JsonRejection>,
) -> ApiResult<Dashboard> {
    let definition = read_dashboard_definition(payload)?;
    api.run_blocking_query(move |api| Ok(api.state.create_dashboard(&definition)?))
        .await
}

/// One saved dashboard.
#[utoipa::path(
    get,
    path = "/api/dashboards/{id}",
    params(("id" = i64, Path, description = "The ID of the dashboard")),
    responses(
        (status = 200, body = Dashboard),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub async fn get_dashboard(State(api): State<Api>, Path(id): Path<String>) -> ApiResult<Dashboard> {
    let id = parse_dashboard_id(&id)?;
    api.run_blocking_query(move |api| {
        api.state
            .find_dashboard(id)?
            .ok_or_else(|| report_missing_dashboard(id))
    })
    .await
}

/// Replaces what a saved dashboard shows.
#[utoipa::path(
    put,
    path = "/api/dashboards/{id}",
    params(("id" = i64, Path, description = "The ID of the dashboard")),
    request_body = DashboardDefinition,
    responses(
        (status = 200, body = Dashboard),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub async fn replace_dashboard(
    State(api): State<Api>,
    Path(id): Path<String>,
    payload: Result<Json<DashboardDefinition>, JsonRejection>,
) -> ApiResult<Dashboard> {
    let id = parse_dashboard_id(&id)?;
    let definition = read_dashboard_definition(payload)?;
    api.run_blocking_query(move |api| {
        api.state
            .replace_dashboard(id, &definition)?
            .ok_or_else(|| report_missing_dashboard(id))
    })
    .await
}

/// Deletes a saved dashboard, and lists the ones left.
#[utoipa::path(
    delete,
    path = "/api/dashboards/{id}",
    params(("id" = i64, Path, description = "The ID of the dashboard")),
    responses(
        (status = 200, body = DashboardList),
        (status = 400, body = ErrorBody),
        (status = 404, body = ErrorBody),
    ),
)]
pub async fn delete_dashboard(
    State(api): State<Api>,
    Path(id): Path<String>,
) -> ApiResult<DashboardList> {
    let id = parse_dashboard_id(&id)?;
    api.run_blocking_query(move |api| {
        if !api.state.delete_dashboard(id)? {
            return Err(report_missing_dashboard(id));
        }
        Ok(api.state.list_dashboards()?)
    })
    .await
}
