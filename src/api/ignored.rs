//! The map's ignore list: systems everyone routes around and never maps by flying.

use super::extract::Credentials;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};

use super::ApiResult;
use super::extract::{ShareQuery, acting_on, read_map_as};
use crate::auth::AppState;
use crate::maps::ignored::{
    AddIgnoredSystem, ClearIgnoredSystems, IgnoredSystem, RemoveIgnoredSystem,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/maps/{id}/ignored", get(list_ignored_systems))
        .route("/api/maps/{id}/ignored/add", post(add_ignored_system))
        .route("/api/maps/{id}/ignored/remove", post(remove_ignored_system))
        .route("/api/maps/{id}/ignored/clear", post(clear_ignored_systems))
}

/// `GET /api/maps/{id}/ignored`: the systems this map keeps out of the way.
pub async fn list_ignored_systems(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<IgnoredSystem>> {
    read_map_as(&state, &creds, map_id, &share).await?;
    let rows = crate::maps::ignored::read_ignored_systems(&state.db, map_id).await?;
    Ok(Json(rows))
}

pub async fn add_ignored_system(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddIgnoredSystem>,
) -> ApiResult<IgnoredSystem> {
    let actor = acting_on(&state.db, &creds, map_id, cmd.map_id).await?;
    let row = crate::maps::ignored::add_ignored_system(&state.db, actor, cmd).await?;
    Ok(Json(row))
}

pub async fn remove_ignored_system(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveIgnoredSystem>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &creds, map_id, cmd.map_id).await?;
    crate::maps::ignored::remove_ignored_system(&state.db, actor, cmd).await?;
    Ok(Json(()))
}

pub async fn clear_ignored_systems(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Json(cmd): Json<ClearIgnoredSystems>,
) -> ApiResult<u64> {
    let actor = acting_on(&state.db, &creds, map_id, cmd.map_id).await?;
    let count = crate::maps::ignored::clear_ignored_systems(&state.db, actor, cmd).await?;
    Ok(Json(count))
}
