//! The map's event log and the moves through it: undo, redo, and jumping to any step.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;

use super::ApiResult;
use super::extract::{check_map_id, require_actor};
use crate::auth::AppState;
use crate::maps::MapEvent;
use crate::maps::events_log::{GotoMapEvent, MapHistory, MapIdBody};

/// The routes this module owns, merged into the API router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/maps/{id}/events", get(list_map_events))
        .route("/api/maps/{id}/events/undo", post(undo_map_event))
        .route("/api/maps/{id}/events/redo", post(redo_map_event))
        .route("/api/maps/{id}/events/goto", post(goto_map_event))
}

/// `GET /api/maps/{id}/events` — the map's history tree and where it currently sits. Viewer+.
pub async fn list_map_events(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<MapHistory> {
    let actor = require_actor(&state.db, &jar).await?;
    let history = crate::maps::events_log::list_history(&state.db, actor, map_id).await?;
    Ok(Json(history))
}

/// `POST /api/maps/{id}/events/undo` — step back to the previous point in the history.
/// Member+. Moving the cursor can touch anything the steps it crosses did, so it publishes
/// `HistoryChanged` and clients refetch rather than trying to reconstruct a targeted event.
pub async fn undo_map_event(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<MapIdBody>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::events_log::undo(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::HistoryChanged { map_id });
    Ok(Json(()))
}

/// `POST /api/maps/{id}/events/redo` — step forward onto the most recent next point. Member+.
pub async fn redo_map_event(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<MapIdBody>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::events_log::redo(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::HistoryChanged { map_id });
    Ok(Json(()))
}

/// `POST /api/maps/{id}/events/goto` — move the map onto any step, including one on a
/// branch that was left behind by an undo. Member+.
pub async fn goto_map_event(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<GotoMapEvent>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::events_log::goto(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::HistoryChanged { map_id });
    Ok(Json(()))
}
