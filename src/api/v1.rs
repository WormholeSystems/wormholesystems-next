//! The public API for scripts and other tools, in the shape the legacy application served
//! so a client written against it can be pointed here. Documented in
//! [`docs/api.md`](../../docs/api.md). Nothing in here is a rule: every write goes through
//! the same commands the map screen uses, and every read through the same reader.

use std::collections::HashMap;

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::extract::{Credentials, ShareQuery, require_actor};
use super::{ApiError, ApiResult};
use crate::auth::AppState;
use crate::maps::map::{GetMap, UpdateMap, read_map};
use crate::maps::signatures::read_signatures;
use crate::maps::solar_system::{
    AddSystem, MoveSystem, RemoveSystem, SetAlias, SetNotes, SetOccupier, SetPinned, SetStatus,
};
use crate::maps::{
    Actor, MapConnection, MapSolarSystem, MapSystemView, MapView, MassStatus, Role, Signature,
    SignatureGroup, SystemStatus, TimeStatus, WormholeSize,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/sovereignties", get(sovereignties))
        .route("/api/v1/maps", get(list_maps))
        .route(
            "/api/v1/maps/{id}",
            get(show_map).put(update_map).patch(update_map),
        )
        .route(
            "/api/v1/map-solarsystems",
            axum::routing::post(create_map_solarsystem),
        )
        .route(
            "/api/v1/map-solarsystems/{id}",
            get(show_map_solarsystem)
                .put(update_map_solarsystem)
                .patch(update_map_solarsystem)
                .delete(delete_map_solarsystem),
        )
}

/// The `{ "data": ... }` envelope the legacy API wrapped everything in.
#[derive(Serialize)]
pub struct Data<T> {
    pub data: T,
}

#[derive(Serialize)]
pub struct Message {
    pub message: &'static str,
}

#[derive(Serialize)]
pub struct MessageWith<T> {
    pub message: &'static str,
    pub data: T,
}

// ---------------------------------------------------------------------------------------
// Sovereignty
// ---------------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SovereigntyEntry {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alliance: Option<TickeredEntity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corporation: Option<TickeredEntity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction: Option<NamedEntity>,
}

#[derive(Serialize)]
pub struct TickeredEntity {
    pub id: i64,
    pub name: String,
    pub ticker: String,
}

#[derive(Serialize)]
pub struct NamedEntity {
    pub id: i64,
    pub name: String,
}

/// `GET /api/v1/sovereignties`: who holds every claimed system, keyed by system id. Public,
/// and cacheable for a day: sovereignty moves slower than that.
pub async fn sovereignties(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = sqlx::query!(
        r#"select sov.solar_system_id,
                  al.id as "alliance_id?", al.name as "alliance_name?",
                  al.ticker as "alliance_ticker?",
                  co.id as "corporation_id?", co.name as "corporation_name?",
                  co.ticker as "corporation_ticker?",
                  f.id as "faction_id?", f.name as "faction_name?"
           from system_sovereignty sov
           left join alliances al on al.id = sov.alliance_id
           left join corporations co on co.id = sov.corporation_id
           left join factions f on f.id = sov.faction_id
           where sov.alliance_id is not null or sov.corporation_id is not null
              or sov.faction_id is not null"#
    )
    .fetch_all(&state.db)
    .await?;

    let entries: HashMap<String, SovereigntyEntry> = rows
        .into_iter()
        .map(|row| {
            let alliance = row.alliance_id.map(|id| TickeredEntity {
                id,
                name: row.alliance_name.unwrap_or_default(),
                ticker: row.alliance_ticker.unwrap_or_default(),
            });
            let corporation = row.corporation_id.map(|id| TickeredEntity {
                id,
                name: row.corporation_name.unwrap_or_default(),
                ticker: row.corporation_ticker.unwrap_or_default(),
            });
            let faction = row.faction_id.map(|id| NamedEntity {
                id,
                name: row.faction_name.unwrap_or_default(),
            });
            (
                row.solar_system_id.to_string(),
                SovereigntyEntry {
                    id: row.solar_system_id,
                    alliance,
                    corporation,
                    faction,
                },
            )
        })
        .collect();

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=86400")],
        Json(entries),
    )
        .into_response())
}

// ---------------------------------------------------------------------------------------
// Maps
// ---------------------------------------------------------------------------------------

/// A map as the legacy API served it: the settings a script may care about, the placed
/// systems with their counts, the connections, and who owns it.
#[derive(Serialize)]
pub struct MapResource {
    pub id: i64,
    pub name: String,
    pub home_solarsystem_id: Option<i64>,
    pub rally_solarsystem_id: Option<i64>,
    pub layout: &'static str,
    pub allow_layout_override: bool,
    pub bookmark_format_wormhole: String,
    pub bookmark_format_kspace: String,
    pub bookmark_format_return: String,
    pub bookmark_alias_scheme: &'static str,
    pub bookmark_ignored_alias: String,
    pub map_solarsystems: Vec<MapSolarsystemResource>,
    pub map_connections: Vec<MapConnectionResource>,
    pub owner: Option<Owner>,
}

#[derive(Serialize)]
pub struct Owner {
    pub id: i64,
    pub character_name: String,
}

#[derive(Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize)]
pub struct MapSolarsystemResource {
    pub id: i64,
    pub map_id: i64,
    pub alias: Option<String>,
    pub status: SystemStatus,
    pub occupier_alias: Option<String>,
    pub position: Position,
    pub pinned: bool,
    pub solarsystem_id: i64,
    pub signatures_count: usize,
    pub uncategorized_signatures_count: usize,
    pub wormhole_signatures_count: usize,
    pub map_connections_count: usize,
    pub threat_level: Option<&'static str>,
}

#[derive(Serialize)]
pub struct MapConnectionResource {
    pub id: i64,
    pub from_map_solarsystem_id: i64,
    pub to_map_solarsystem_id: i64,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub preserve_mass: bool,
    pub mass_status: Option<&'static str>,
    pub lifetime_status: Option<&'static str>,
    pub lifetime_status_updated_at: Option<DateTime<Utc>>,
    pub ship_size: Option<&'static str>,
    pub jumps_mass_sum: i64,
    pub jumps_count: i64,
    pub signatures: Vec<ConnectionSignatureResource>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct ConnectionSignatureResource {
    pub id: i64,
    pub signature_id: String,
    pub map_solarsystem_id: i64,
    pub mass_status: Option<&'static str>,
    pub lifetime_status: Option<&'static str>,
    pub lifetime_status_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The legacy vocabulary for a hole's remaining life, which named the healthy state.
fn legacy_lifetime(status: TimeStatus) -> &'static str {
    match status {
        TimeStatus::Stable => "healthy",
        TimeStatus::Eol => "eol",
        TimeStatus::Critical => "critical",
    }
}

fn legacy_mass(status: MassStatus) -> &'static str {
    match status {
        MassStatus::Stable => "fresh",
        MassStatus::Reduced => "reduced",
        MassStatus::Critical => "critical",
    }
}

fn legacy_size(size: WormholeSize) -> &'static str {
    match size {
        WormholeSize::Small => "frigate",
        WormholeSize::Medium => "medium",
        WormholeSize::Large => "large",
        WormholeSize::Xl => "xlarge",
    }
}

/// A placement a script can address. A ghost has no system yet, and the legacy API had no
/// such thing, so those are left out.
struct PlacedSystem<'a> {
    id: i64,
    map_id: i64,
    solar_system_id: i64,
    position: Position,
    alias: Option<&'a str>,
    is_home: bool,
    is_rally: bool,
    is_pinned: bool,
    status: SystemStatus,
    occupying_group: Option<&'a str>,
    threat_level: Option<&'static str>,
}

fn placed_systems(view: &MapView) -> impl Iterator<Item = PlacedSystem<'_>> {
    view.systems.iter().filter_map(|system| match system {
        MapSystemView::System {
            id,
            map_id,
            solar_system_id,
            position_x,
            position_y,
            alias,
            is_home,
            is_rally,
            is_pinned,
            status,
            occupying_group,
            threat_level,
            ..
        } => Some(PlacedSystem {
            id: *id,
            map_id: *map_id,
            solar_system_id: *solar_system_id,
            position: Position {
                x: *position_x,
                y: *position_y,
            },
            alias: alias.as_deref(),
            is_home: *is_home,
            is_rally: *is_rally,
            is_pinned: *is_pinned,
            status: *status,
            occupying_group: occupying_group.as_deref(),
            threat_level: threat_level.map(|t| t.as_str()),
        }),
        MapSystemView::Ghost { .. } => None,
    })
}

fn connection_resource(
    connection: &MapConnection,
    signatures: &[Signature],
    placement_of: &HashMap<i64, i64>,
) -> MapConnectionResource {
    MapConnectionResource {
        id: connection.id,
        from_map_solarsystem_id: connection.from_system,
        to_map_solarsystem_id: connection.to_system,
        kind: connection.kind.as_str(),
        preserve_mass: connection.preserve_mass,
        mass_status: connection.mass_status.map(legacy_mass),
        lifetime_status: connection.time_status.map(legacy_lifetime),
        lifetime_status_updated_at: connection.time_status_updated_at,
        ship_size: connection.size.map(legacy_size),
        jumps_mass_sum: connection.jumps_mass_sum,
        jumps_count: connection.jumps_count,
        signatures: signatures
            .iter()
            .filter(|s| s.connection_id == Some(connection.id))
            .filter_map(|s| {
                Some(ConnectionSignatureResource {
                    id: s.id,
                    signature_id: s.signature_id.clone(),
                    map_solarsystem_id: *placement_of.get(&s.solar_system_id)?,
                    mass_status: s.mass_status.map(legacy_mass),
                    lifetime_status: s.time_status.map(legacy_lifetime),
                    lifetime_status_updated_at: s.time_status_updated_at,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                })
            })
            .collect(),
        created_at: connection.created_at,
        updated_at: connection.updated_at,
    }
}

async fn owner_of(pool: &sqlx::PgPool, map_id: i64) -> Result<Option<Owner>, ApiError> {
    let row = sqlx::query!(
        "select c.id, c.name
         from map_access_live a
         join characters c on c.id = a.subject_id
         where a.map_id = $1 and a.role = 'owner' and a.subject_type = 'character'
         order by a.id
         limit 1",
        map_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Owner {
        id: r.id,
        character_name: r.name,
    }))
}

async fn map_resource(pool: &sqlx::PgPool, view: MapView) -> Result<MapResource, ApiError> {
    let map_id = view.map.id;
    let signatures = read_signatures(pool, map_id).await?;
    let placement_of: HashMap<i64, i64> = placed_systems(&view)
        .map(|s| (s.solar_system_id, s.id))
        .collect();

    let map_solarsystems = placed_systems(&view)
        .map(|system| {
            let mine: Vec<&Signature> = signatures
                .iter()
                .filter(|s| s.solar_system_id == system.solar_system_id)
                .collect();
            MapSolarsystemResource {
                id: system.id,
                map_id: system.map_id,
                alias: system.alias.map(str::to_string),
                status: system.status,
                occupier_alias: system.occupying_group.map(str::to_string),
                position: system.position,
                pinned: system.is_pinned,
                solarsystem_id: system.solar_system_id,
                signatures_count: mine.len(),
                uncategorized_signatures_count: mine
                    .iter()
                    .filter(|s| s.group == SignatureGroup::Unknown)
                    .count(),
                wormhole_signatures_count: mine
                    .iter()
                    .filter(|s| s.group == SignatureGroup::Wormhole)
                    .count(),
                map_connections_count: view
                    .connections
                    .iter()
                    .filter(|c| c.from_system == system.id || c.to_system == system.id)
                    .count(),
                threat_level: system.threat_level,
            }
        })
        .collect();

    let map_connections = view
        .connections
        .iter()
        .map(|c| connection_resource(c, &signatures, &placement_of))
        .collect();

    let home_solarsystem_id = placed_systems(&view)
        .find(|s| s.is_home)
        .map(|s| s.solar_system_id);
    let rally_solarsystem_id = placed_systems(&view)
        .find(|s| s.is_rally)
        .map(|s| s.solar_system_id);

    Ok(MapResource {
        id: map_id,
        name: view.map.name,
        home_solarsystem_id,
        rally_solarsystem_id,
        layout: view.map.layout.as_str(),
        allow_layout_override: view.map.allow_layout_override,
        bookmark_format_wormhole: view.map.naming.bookmark_wormhole,
        bookmark_format_kspace: view.map.naming.bookmark_kspace,
        bookmark_format_return: view.map.naming.bookmark_return,
        bookmark_alias_scheme: view.map.naming.alias_scheme.as_str(),
        bookmark_ignored_alias: view.map.naming.ignored_alias,
        map_solarsystems,
        map_connections,
        owner: owner_of(pool, map_id).await?,
    })
}

/// The map through the same reader the map screen uses, so a token reads exactly what its
/// user could see signed in.
async fn read_as(state: &AppState, creds: &Credentials, map_id: i64) -> Result<MapView, ApiError> {
    let reader =
        super::extract::read_map_as(state, creds, map_id, &ShareQuery { share: None }).await?;
    Ok(read_map(&state.db, reader, GetMap { map_id }).await?)
}

#[derive(Deserialize)]
pub struct MapListQuery {
    #[serde(default)]
    pub search: Option<String>,
}

/// `GET /api/v1/maps?search=`, every map the caller can reach, optionally narrowed by name.
pub async fn list_maps(
    State(state): State<AppState>,
    creds: Credentials,
    Query(query): Query<MapListQuery>,
) -> ApiResult<Data<Vec<MapResource>>> {
    let actor = require_actor(&state.db, &creds).await?;
    let needle = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let mut maps = Vec::new();
    for (map, _) in crate::maps::map::list_maps(&state.db, actor.user_id).await? {
        if let Some(needle) = &needle
            && !map.name.to_lowercase().contains(needle)
        {
            continue;
        }
        let view = read_as(&state, &creds, map.id).await?;
        maps.push(map_resource(&state.db, view).await?);
    }
    Ok(Json(Data { data: maps }))
}

/// `GET /api/v1/maps/{id}`.
pub async fn show_map(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
) -> ApiResult<Data<MapResource>> {
    let view = read_as(&state, &creds, map_id).await?;
    Ok(Json(Data {
        data: map_resource(&state.db, view).await?,
    }))
}

#[derive(Deserialize)]
pub struct UpdateMapBody {
    pub name: String,
}

/// `PUT /api/v1/maps/{id}`, rename it. Manager+, as on the settings page.
pub async fn update_map(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Json(body): Json<UpdateMapBody>,
) -> ApiResult<MessageWith<MapResource>> {
    let actor = require_actor(&state.db, &creds).await?;
    crate::maps::map::update_map(
        &state.db,
        actor,
        UpdateMap {
            map_id,
            name: Some(body.name),
            description: None,
            image_url: None,
            naming: None,
            ghost_unlinked_wormholes: None,
            layout: None,
            allow_layout_override: None,
            is_public: None,
        },
    )
    .await?;
    let view = read_as(&state, &creds, map_id).await?;
    Ok(Json(MessageWith {
        message: "Map updated successfully.",
        data: map_resource(&state.db, view).await?,
    }))
}

// ---------------------------------------------------------------------------------------
// Map solar systems
// ---------------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SelectedMapSolarsystemResource {
    pub id: i64,
    pub map_id: i64,
    pub solarsystem_id: i64,
    pub alias: Option<String>,
    pub status: SystemStatus,
    pub occupier_alias: Option<String>,
    /// Member-gated intel; `null` for a viewer.
    pub notes: Option<String>,
    pub position: Position,
    pub is_pinned: bool,
    pub map_connections: Vec<MapConnectionResource>,
    pub signatures: Vec<SignatureResource>,
}

#[derive(Serialize)]
pub struct SignatureResource {
    pub id: i64,
    pub signature_id: String,
    pub map_solarsystem_id: i64,
    pub signature_type_id: Option<i64>,
    pub signature_category: &'static str,
    pub raw_type_name: Option<String>,
    pub map_connection_id: Option<i64>,
    pub mass_status: Option<&'static str>,
    pub ship_size: Option<&'static str>,
    pub lifetime: Option<&'static str>,
    pub lifetime_updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Which map a placement is on. `NotFound` here and `NotFound` from the access check look
/// the same, so an id that is not yours is indistinguishable from one that does not exist.
async fn map_of_placement(pool: &sqlx::PgPool, placement_id: i64) -> Result<i64, ApiError> {
    let map_id = sqlx::query_scalar!(
        "select map_id from map_solar_systems where id = $1",
        placement_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(crate::maps::MapError::NotFound)?;
    Ok(map_id)
}

/// `GET /api/v1/map-solarsystems/{id}`, one placement with its signatures and connections.
pub async fn show_map_solarsystem(
    State(state): State<AppState>,
    creds: Credentials,
    Path(placement_id): Path<i64>,
) -> ApiResult<Data<SelectedMapSolarsystemResource>> {
    let map_id = map_of_placement(&state.db, placement_id).await?;
    let view = read_as(&state, &creds, map_id).await?;
    let system = placed_systems(&view)
        .find(|s| s.id == placement_id)
        .ok_or(crate::maps::MapError::NotFound)?;

    let signatures = read_signatures(&state.db, map_id).await?;
    let placement_of: HashMap<i64, i64> = placed_systems(&view)
        .map(|s| (s.solar_system_id, s.id))
        .collect();
    let notes = if view.role >= Role::Member {
        sqlx::query_scalar!(
            "select notes from map_solar_system_details
             where map_id = $1 and solar_system_id = $2",
            map_id,
            system.solar_system_id,
        )
        .fetch_optional(&state.db)
        .await?
        .flatten()
    } else {
        None
    };

    let resource = SelectedMapSolarsystemResource {
        id: system.id,
        map_id,
        solarsystem_id: system.solar_system_id,
        alias: system.alias.map(str::to_string),
        status: system.status,
        occupier_alias: system.occupying_group.map(str::to_string),
        notes,
        position: system.position,
        is_pinned: system.is_pinned,
        map_connections: view
            .connections
            .iter()
            .filter(|c| c.from_system == placement_id || c.to_system == placement_id)
            .map(|c| connection_resource(c, &signatures, &placement_of))
            .collect(),
        signatures: signatures
            .iter()
            .filter(|s| s.solar_system_id == system.solar_system_id)
            .map(|s| SignatureResource {
                id: s.id,
                signature_id: s.signature_id.clone(),
                map_solarsystem_id: placement_id,
                signature_type_id: s.signature_type_id,
                signature_category: s.group.as_str(),
                raw_type_name: s.name.clone(),
                map_connection_id: s.connection_id,
                mass_status: s.mass_status.map(legacy_mass),
                ship_size: s.size.map(legacy_size),
                lifetime: s.time_status.map(legacy_lifetime),
                lifetime_updated_at: s.time_status_updated_at,
                created_at: s.created_at,
                updated_at: s.updated_at,
            })
            .collect(),
    };
    Ok(Json(Data { data: resource }))
}

#[derive(Deserialize)]
pub struct CreateMapSolarsystemBody {
    pub map_id: i64,
    pub solarsystem_id: i64,
    pub position_x: f64,
    pub position_y: f64,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub occupier_alias: Option<String>,
    #[serde(default)]
    pub status: Option<SystemStatus>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

/// `POST /api/v1/map-solarsystems`, place a system. Each optional detail is its own
/// command afterwards, so the history reads as it would had somebody done it by hand.
pub async fn create_map_solarsystem(
    State(state): State<AppState>,
    creds: Credentials,
    Json(body): Json<CreateMapSolarsystemBody>,
) -> Result<(StatusCode, Json<MessageWith<MapSolarSystem>>), ApiError> {
    let actor = require_actor(&state.db, &creds).await?;
    let placed = crate::maps::solar_system::add_system(
        &state.db,
        actor,
        AddSystem {
            map_id: body.map_id,
            solar_system_id: body.solarsystem_id,
            x: body.position_x,
            y: body.position_y,
            alias: body.alias,
        },
    )
    .await?;
    apply_details(
        &state.db,
        actor,
        body.map_id,
        placed.id,
        Details {
            occupier_alias: body.occupier_alias.map(Some),
            status: body.status,
            pinned: body.pinned,
            ..Details::default()
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(MessageWith {
            message: "Solarsystem created successfully.",
            data: placed,
        }),
    ))
}

/// The per-field edits a placement update splits into. `Some(None)` clears a text field.
#[derive(Default)]
struct Details {
    alias: Option<Option<String>>,
    occupier_alias: Option<Option<String>>,
    notes: Option<Option<String>>,
    position: Option<(f64, f64)>,
    status: Option<SystemStatus>,
    pinned: Option<bool>,
}

async fn apply_details(
    pool: &sqlx::PgPool,
    actor: Actor,
    map_id: i64,
    map_solar_system_id: i64,
    details: Details,
) -> Result<(), ApiError> {
    if let Some(alias) = details.alias {
        crate::maps::solar_system::set_alias(
            pool,
            actor,
            SetAlias {
                map_id,
                map_solar_system_id,
                alias,
            },
        )
        .await?;
    }
    if let Some(occupier) = details.occupier_alias {
        crate::maps::solar_system::set_occupier(
            pool,
            actor,
            SetOccupier {
                map_id,
                map_solar_system_id,
                occupier,
            },
        )
        .await?;
    }
    if let Some(notes) = details.notes {
        crate::maps::solar_system::set_notes(
            pool,
            actor,
            SetNotes {
                map_id,
                map_solar_system_id,
                notes,
            },
        )
        .await?;
    }
    if let Some((x, y)) = details.position {
        crate::maps::solar_system::move_system(
            pool,
            actor,
            MoveSystem {
                map_id,
                map_solar_system_id,
                x,
                y,
            },
        )
        .await?;
    }
    if let Some(status) = details.status {
        crate::maps::solar_system::set_status(
            pool,
            actor,
            SetStatus {
                map_id,
                map_solar_system_id,
                status,
            },
        )
        .await?;
    }
    if let Some(value) = details.pinned {
        crate::maps::solar_system::set_pinned(
            pool,
            actor,
            SetPinned {
                map_id,
                map_solar_system_id,
                value,
            },
        )
        .await?;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct UpdateMapSolarsystemBody {
    #[serde(default, deserialize_with = "crate::maps::double_option")]
    pub alias: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::maps::double_option")]
    pub occupier_alias: Option<Option<String>>,
    #[serde(default, deserialize_with = "crate::maps::double_option")]
    pub notes: Option<Option<String>>,
    #[serde(default)]
    pub position_x: Option<f64>,
    #[serde(default)]
    pub position_y: Option<f64>,
    #[serde(default)]
    pub status: Option<SystemStatus>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

/// `PUT /api/v1/map-solarsystems/{id}`. An absent field is left alone; `null` clears it.
/// A position needs both halves.
pub async fn update_map_solarsystem(
    State(state): State<AppState>,
    creds: Credentials,
    Path(placement_id): Path<i64>,
    Json(body): Json<UpdateMapSolarsystemBody>,
) -> ApiResult<Message> {
    let position = match (body.position_x, body.position_y) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "position_x and position_y go together",
            ));
        }
    };
    let actor = require_actor(&state.db, &creds).await?;
    let map_id = map_of_placement(&state.db, placement_id).await?;
    apply_details(
        &state.db,
        actor,
        map_id,
        placement_id,
        Details {
            alias: body.alias,
            occupier_alias: body.occupier_alias,
            notes: body.notes,
            position,
            status: body.status,
            pinned: body.pinned,
        },
    )
    .await?;
    Ok(Json(Message {
        message: "Solarsystem updated successfully.",
    }))
}

/// `DELETE /api/v1/map-solarsystems/{id}`.
pub async fn delete_map_solarsystem(
    State(state): State<AppState>,
    creds: Credentials,
    Path(placement_id): Path<i64>,
) -> ApiResult<Message> {
    let actor = require_actor(&state.db, &creds).await?;
    let map_id = map_of_placement(&state.db, placement_id).await?;
    crate::maps::solar_system::remove_system(
        &state.db,
        actor,
        RemoveSystem {
            map_id,
            map_solar_system_id: placement_id,
        },
    )
    .await?;
    Ok(Json(Message {
        message: "Solarsystem deleted successfully.",
    }))
}
