//! The map's watchlist: systems somebody wants a standing route to.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;

use super::ApiResult;
use super::extract::{ShareQuery, check_map_id, read_map_as, require_actor};
use crate::auth::AppState;
use crate::maps::MapEvent;
use crate::maps::watchlist::{
    AddWatchlistEntry, RemoveWatchlistEntry, SetWatchlistPinned, WatchlistEntry,
};

/// The routes this module owns, merged into the API router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/maps/{id}/watchlist", get(list_watchlist))
        .route("/api/maps/{id}/watchlist/add", post(add_watchlist_entry))
        .route(
            "/api/maps/{id}/watchlist/set-pinned",
            post(set_watchlist_pinned),
        )
        .route(
            "/api/maps/{id}/watchlist/remove",
            post(remove_watchlist_entry),
        )
}

/// `GET /api/maps/{id}/watchlist` — the map's tracked destinations.
pub async fn list_watchlist(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<WatchlistEntry>> {
    read_map_as(&state, &jar, map_id, &share).await?;
    let entries = crate::maps::watchlist::read_watchlist(&state.db, map_id).await?;
    Ok(Json(entries))
}

pub async fn add_watchlist_entry(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddWatchlistEntry>,
) -> ApiResult<WatchlistEntry> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let entry = crate::maps::watchlist::add_watchlist_entry(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::WatchlistChanged { map_id });
    Ok(Json(entry))
}

pub async fn set_watchlist_pinned(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<SetWatchlistPinned>,
) -> ApiResult<WatchlistEntry> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let entry = crate::maps::watchlist::set_watchlist_pinned(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::WatchlistChanged { map_id });
    Ok(Json(entry))
}

pub async fn remove_watchlist_entry(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveWatchlistEntry>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::watchlist::remove_watchlist_entry(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::WatchlistChanged { map_id });
    Ok(Json(()))
}
