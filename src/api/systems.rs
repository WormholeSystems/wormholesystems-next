//! Systems on a map: placing, moving, removing, and the per-system details (alias,
//! status, occupier, home, rally, pinned, notes) that are all the same shape.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;

use super::ApiResult;
use super::extract::{check_map_id, require_actor};
use crate::auth::AppState;
use crate::maps::solar_system::{
    AddSystem, ClearMap, MoveSystem, MoveSystems, RemoveSystem, RemoveSystems, SetAlias, SetHome,
    SetNotes, SetOccupier, SetPinned, SetRally, SetStatus, SystemDetails,
};
use crate::maps::{MapEvent, MapSolarSystem};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/maps/{id}/clear", post(clear_map))
        .route("/api/maps/{id}/systems/add", post(add_system))
        .route(
            "/api/maps/{id}/systems/resolve-ghost",
            post(resolve_ghost_system),
        )
        .route("/api/maps/{id}/systems/move", post(move_systems))
        .route("/api/maps/{id}/systems/move-one", post(move_system))
        .route("/api/maps/{id}/systems/remove", post(remove_systems))
        .route("/api/maps/{id}/systems/remove-one", post(remove_system))
        .route("/api/maps/{id}/systems/set-alias", post(set_alias))
        .route("/api/maps/{id}/systems/set-status", post(set_status))
        .route("/api/maps/{id}/systems/set-occupier", post(set_occupier))
        .route("/api/maps/{id}/systems/set-home", post(set_home))
        .route("/api/maps/{id}/systems/set-rally", post(set_rally))
        .route("/api/maps/{id}/systems/set-pinned", post(set_pinned))
        .route("/api/maps/{id}/systems/set-notes", post(set_notes))
        .route("/api/maps/{id}/systems/{mss}/details", get(system_details))
}

/// `POST /api/maps/{id}/systems/resolve-ghost`, say which system a ghost turned out to
/// be. Merging into an existing placement removes the ghost, so that goes out too.
pub async fn resolve_ghost_system(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<crate::maps::ghost::ResolveGhostSystem>,
) -> ApiResult<MapSolarSystem> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let ghost_id = cmd.map_solar_system_id;
    let placed = crate::maps::ghost::resolve_ghost_system(&state.db, actor, cmd).await?;
    if placed.id != ghost_id {
        state.hub.publish(MapEvent::SystemRemoved {
            map_id,
            map_solar_system_id: ghost_id,
        });
    }
    state.hub.publish(MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: placed.id,
    });
    Ok(Json(placed))
}

pub async fn add_system(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddSystem>,
) -> ApiResult<MapSolarSystem> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let placed = crate::maps::solar_system::add_system(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SystemAdded {
        map_id,
        map_solar_system_id: placed.id,
    });
    Ok(Json(placed))
}

pub async fn move_system(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<MoveSystem>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let mss = cmd.map_solar_system_id;
    crate::maps::solar_system::move_system(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SystemMoved {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(Json(()))
}

pub async fn move_systems(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<MoveSystems>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::solar_system::move_systems(&state.db, actor, cmd).await?;
    // Several systems moved at once → one coarse event rather than N SystemMoved events.
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(()))
}

pub async fn remove_system(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSystem>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let mss = cmd.map_solar_system_id;
    crate::maps::solar_system::remove_system(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SystemRemoved {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(Json(()))
}

pub async fn remove_systems(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSystems>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::solar_system::remove_systems(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(()))
}

pub async fn clear_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<ClearMap>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::solar_system::clear_map(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(()))
}

macro_rules! detail_handler {
    ($name:ident, $cmd:ty, $action:path) => {
        pub async fn $name(
            State(state): State<AppState>,
            jar: CookieJar,
            Path(map_id): Path<i64>,
            Json(cmd): Json<$cmd>,
        ) -> ApiResult<()> {
            check_map_id(map_id, cmd.map_id)?;
            let actor = require_actor(&state.db, &jar).await?;
            let mss = cmd.map_solar_system_id;
            $action(&state.db, actor, cmd).await?;
            state.hub.publish(MapEvent::SystemDetailsChanged {
                map_id,
                map_solar_system_id: mss,
            });
            Ok(Json(()))
        }
    };
}

detail_handler!(set_alias, SetAlias, crate::maps::solar_system::set_alias);

detail_handler!(set_status, SetStatus, crate::maps::solar_system::set_status);

detail_handler!(
    set_occupier,
    SetOccupier,
    crate::maps::solar_system::set_occupier
);

detail_handler!(set_notes, SetNotes, crate::maps::solar_system::set_notes);

detail_handler!(set_home, SetHome, crate::maps::solar_system::set_home);

detail_handler!(set_rally, SetRally, crate::maps::solar_system::set_rally);

detail_handler!(set_pinned, SetPinned, crate::maps::solar_system::set_pinned);

/// `GET /api/maps/{id}/systems/{mss}/details`, member-gated intel (notes). 403 for viewers.
pub async fn system_details(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, mss)): Path<(i64, i64)>,
) -> ApiResult<SystemDetails> {
    let actor = require_actor(&state.db, &jar).await?;
    let details = crate::maps::solar_system::system_details(&state.db, actor, map_id, mss).await?;
    Ok(Json(details))
}
