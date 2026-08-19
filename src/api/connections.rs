//! Connections between mapped systems, the jump log kept against them, and the sweep
//! that clears out the ones nobody has been through in hours.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::extract::{ShareQuery, check_map_id, read_map_as, require_actor};
use super::{ApiError, ApiResult};
use crate::auth::AppState;
use crate::maps::connection::{
    AddConnection, CleanStaleConnections, RemoveConnection, SetConnectionStatus, StaleConnection,
};
use crate::maps::jumps::{
    AddConnectionJump, ConnectionJump, RemoveConnectionJump, UpdateConnectionJump,
};
use crate::maps::{MapConnection, MapEvent};

/// A ship type matched by the manual-jump ship search.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ShipSearchResult {
    pub id: i64,
    pub name: String,
    pub group_name: String,
    /// Hull mass in kg.
    pub mass: Option<f64>,
}

/// The routes this module owns, merged into the API router.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/ships/search", get(search_ships))
        .route("/api/maps/{id}/connections/add", post(add_connection))
        .route(
            "/api/maps/{id}/connections/set-status",
            post(set_connection_status),
        )
        .route("/api/maps/{id}/connections/remove", post(remove_connection))
        .route(
            "/api/maps/{id}/connections/{cid}/jumps",
            get(list_connection_jumps),
        )
        .route(
            "/api/maps/{id}/connections/jumps/add",
            post(add_connection_jump),
        )
        .route(
            "/api/maps/{id}/connections/jumps/update",
            post(update_connection_jump),
        )
        .route(
            "/api/maps/{id}/connections/jumps/remove",
            post(remove_connection_jump),
        )
        .route(
            "/api/maps/{id}/connections/stale",
            get(list_stale_connections),
        )
        .route(
            "/api/maps/{id}/connections/clean-stale",
            post(clean_stale_connections),
        )
}

/// `POST /api/maps/{id}/connections/add`
///
/// The ghost guard sits here rather than in the command, because it is a rule about what a
/// person may draw: raising a ghost creates the one connection an unmapped hole is allowed
/// to have, and that goes through the same command from the inside.
pub async fn add_connection(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddConnection>,
) -> ApiResult<MapConnection> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let ghost_endpoint = sqlx::query_scalar!(
        r#"select exists(
               select 1 from map_solar_systems
               where map_id = $1 and id in ($2, $3) and solar_system_id is null
           ) as "ghost!""#,
        map_id,
        cmd.from_system,
        cmd.to_system,
    )
    .fetch_one(&state.db)
    .await?;
    if ghost_endpoint {
        return Err(ApiError::bad_request(
            "assign a system to that hole before connecting anything to it",
        ));
    }
    let conn = crate::maps::connection::add_connection(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::ConnectionChanged {
        map_id,
        connection_id: conn.id,
    });
    Ok(Json(conn))
}

pub async fn set_connection_status(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<SetConnectionStatus>,
) -> ApiResult<MapConnection> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let connection_id = cmd.connection_id;
    let conn = crate::maps::connection::set_connection_status(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::ConnectionChanged {
        map_id,
        connection_id,
    });
    Ok(Json(conn))
}

pub async fn remove_connection(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveConnection>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let connection_id = cmd.connection_id;
    crate::maps::connection::remove_connection(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::ConnectionChanged {
        map_id,
        connection_id,
    });
    Ok(Json(()))
}

/// `GET /api/maps/{id}/connections/{cid}/jumps` — the latest 10 jump-log rows.
pub async fn list_connection_jumps(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, connection_id)): Path<(i64, i64)>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<ConnectionJump>> {
    read_map_as(&state, &jar, map_id, &share).await?;
    let jumps = crate::maps::jumps::read_jumps(&state.db, map_id, connection_id).await?;
    Ok(Json(jumps))
}

pub async fn add_connection_jump(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddConnectionJump>,
) -> ApiResult<ConnectionJump> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let jump = crate::maps::jumps::add_jump(&state.db, actor, cmd).await?;
    if let Some(connection_id) = jump.connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    Ok(Json(jump))
}

pub async fn update_connection_jump(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UpdateConnectionJump>,
) -> ApiResult<ConnectionJump> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let jump = crate::maps::jumps::update_jump(&state.db, actor, cmd).await?;
    if let Some(connection_id) = jump.connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    Ok(Json(jump))
}

pub async fn remove_connection_jump(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveConnectionJump>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let connection_id = crate::maps::jumps::remove_jump(&state.db, actor, cmd).await?;
    if let Some(connection_id) = connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    Ok(Json(()))
}

/// `GET /api/maps/{id}/connections/stale` — edges that have been critical for over an hour.
pub async fn list_stale_connections(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<StaleConnection>> {
    let actor = require_actor(&state.db, &jar).await?;
    let rows = crate::maps::connection::list_stale_connections(&state.db, actor, map_id).await?;
    Ok(Json(rows))
}

/// `POST /api/maps/{id}/connections/clean-stale` — sweep them, and the placements they
/// orphan, as one undoable change.
pub async fn clean_stale_connections(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<CleanStaleConnections>,
) -> ApiResult<u64> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let removed = crate::maps::connection::clean_stale_connections(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::HistoryChanged { map_id });
    Ok(Json(removed))
}

#[derive(Deserialize)]
pub struct ShipSearchQuery {
    pub q: String,
}

/// `GET /api/ships/search?q=` — published ship types (SDE category 6) by name, with
/// hull mass for the manual-jump form. Reference data, no actor check.
pub async fn search_ships(
    State(state): State<AppState>,
    Query(query): Query<ShipSearchQuery>,
) -> ApiResult<Vec<ShipSearchResult>> {
    let q = query.q.trim();
    if q.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let results = sqlx::query_as!(
        ShipSearchResult,
        r#"select t.id, t.name, g.name as group_name, t.mass
           from types t
           join groups g on g.id = t.group_id
           where g.category_id = 6 and t.published and t.name ilike '%' || $1 || '%'
           order by t.name limit 25"#,
        q,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(results))
}
