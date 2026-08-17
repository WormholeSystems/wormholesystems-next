//! The JSON endpoint handlers. Reads are GETs; commands are POSTs whose bodies are the
//! command structs from [`crate::maps`] (JSON, so the `Option<Option<_>>` "absent = leave,
//! null = clear" fields work).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use super::AccessSubject;
use super::{
    ApiError, ApiResult, CharacterRef, CharacterStatus, CharacterSummary, EveScoutEdge,
    MapCharacter, MapEntry, MapSearchHit, MapUserSettings, ShipSearchResult, SignatureCatalog,
    SignatureCategoryInfo, SignatureTypeInfo, SystemSearchResult, ThreatAnalysis, ThreatEntity,
    UpdateMapUserSettings, check_map_id, require_actor, session_actor, session_id,
};
use crate::auth::AppState;
use crate::maps::access::{AccessEntry, RevokeAccess, SetAccess};
use crate::maps::connection::{
    AddConnection, CleanStaleConnections, RemoveConnection, SetConnectionStatus, StaleConnection,
};
use crate::maps::events_log::{GotoMapEvent, MapHistory, MapIdBody};
use crate::maps::jumps::{
    AddConnectionJump, ConnectionJump, RemoveConnectionJump, UpdateConnectionJump,
};
use crate::maps::map::UpdateMap;
use crate::maps::signatures::{
    AddSignature, LinkSignature, PasteSignatures, RemoveSignature, RemoveSignatures,
    UnlinkSignature, UpdateSignature,
};
use crate::maps::solar_system::{
    AddSystem, ClearMap, MoveSystem, MoveSystems, RemoveSystem, RemoveSystems, SetAlias, SetHome,
    SetNotes, SetOccupier, SetPinned, SetRally, SetStatus, SystemDetails,
};
use crate::maps::watchlist::{
    AddWatchlistEntry, RemoveWatchlistEntry, SetWatchlistPinned, WatchlistEntry,
};
use crate::maps::{
    EffectModifier, GridConfig, Map, MapConnection, MapEvent, MapSolarSystem, MapView, Signature,
};

// --- Auth / identity ---

/// `GET /api/me` — who's signed in, if anyone.
pub async fn me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Option<CharacterSummary>> {
    let Some(actor) = session_actor(&state.db, &jar).await? else {
        return Ok(Json(None));
    };
    let row = sqlx::query!(
        "select name from characters where id = $1",
        actor.character_id
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(row.map(|r| CharacterSummary {
        character_id: actor.character_id,
        name: r.name,
    })))
}

/// `GET /api/me/status` — live status of the active character (online / system / ship).
pub async fn me_status(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Option<CharacterStatus>> {
    let Some(actor) = session_actor(&state.db, &jar).await? else {
        return Ok(Json(None));
    };
    let row = sqlx::query!(
        r#"select s.online,
                  s.ship_type_id,
                  ss.name as "solar_system?",
                  s.ship_name,
                  t.name as "ship_type?"
           from character_status s
           left join solar_systems ss on ss.id = s.solar_system_id
           left join types t on t.id = s.ship_type_id
           where s.character_id = $1"#,
        actor.character_id,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(row.map(|r| CharacterStatus {
        online: r.online,
        solar_system: r.solar_system,
        ship_type_id: r.ship_type_id,
        ship_name: r.ship_name,
        ship_type: r.ship_type,
    })))
}

/// `GET /api/me/characters` — the user's characters, marking the active one.
pub async fn my_characters(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Vec<CharacterRef>> {
    let actor = require_actor(&state.db, &jar).await?;
    let rows = sqlx::query!(
        r#"select c.id, c.name, coalesce(s.online, false) as "online!",
                  case when s.online then s.solar_system_id end as "solar_system_id?"
           from characters c
           left join character_status s on s.character_id = c.id
           where c.user_id = $1 order by c.name"#,
        actor.user_id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| CharacterRef {
                character_id: r.id,
                name: r.name,
                is_active: r.id == actor.character_id,
                online: r.online,
                solar_system_id: r.solar_system_id,
            })
            .collect(),
    ))
}

#[derive(Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CharacterIdBody {
    pub character_id: i64,
}

/// `POST /api/me/switch-character` — switch the session's active character.
pub async fn switch_character(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<CharacterIdBody>,
) -> ApiResult<()> {
    let Some(session_id) = session_id(&jar) else {
        return Err(ApiError::unauthorized());
    };
    let ok =
        crate::session::set_active_character(&state.db, &session_id, body.character_id).await?;
    if !ok {
        return Err(ApiError::bad_request("that character isn't yours"));
    }
    Ok(Json(()))
}

/// `POST /api/me/remove-character` — remove one of the user's characters. Refuses to remove
/// the last one. If it's the active character, the session switches to another first (so
/// the session isn't cascade-deleted).
pub async fn remove_character(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<CharacterIdBody>,
) -> ApiResult<()> {
    let Some(session_id) = session_id(&jar) else {
        return Err(ApiError::unauthorized());
    };
    let actor = require_actor(&state.db, &jar).await?;
    let character_id = body.character_id;

    let count = sqlx::query_scalar!(
        "select count(*) from characters where user_id = $1",
        actor.user_id,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);
    if count <= 1 {
        return Err(ApiError::bad_request("can't remove your only character"));
    }

    let owns = sqlx::query_scalar!(
        "select exists(select 1 from characters where id = $1 and user_id = $2)",
        character_id,
        actor.user_id,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);
    if !owns {
        return Err(ApiError::bad_request("that character isn't yours"));
    }

    // Switch away before deleting the active one (sessions FK-cascade on the character).
    if character_id == actor.character_id
        && let Some(other) = sqlx::query_scalar!(
            "select id from characters where user_id = $1 and id <> $2
             order by is_preferred desc, id limit 1",
            actor.user_id,
            character_id,
        )
        .fetch_optional(&state.db)
        .await?
    {
        crate::session::set_active_character(&state.db, &session_id, other).await?;
    }

    sqlx::query!(
        "delete from characters where id = $1 and user_id = $2",
        character_id,
        actor.user_id,
    )
    .execute(&state.db)
    .await?;
    Ok(Json(()))
}

#[derive(Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetWaypointBody {
    pub character_id: i64,
    pub destination_id: i64,
    #[serde(default)]
    #[ts(optional)]
    pub add_to_beginning: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub clear_other_waypoints: Option<bool>,
}

#[derive(Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetWaypointAllBody {
    pub destination_id: i64,
    #[serde(default)]
    #[ts(optional)]
    pub add_to_beginning: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub clear_other_waypoints: Option<bool>,
}

/// Set an in-game autopilot waypoint for one character via ESI. Maps a missing waypoint
/// scope to 409 (the character must re-consent) and other ESI failures to 502.
async fn esi_waypoint(
    state: &AppState,
    character_id: i64,
    destination_id: i64,
    add_to_beginning: bool,
    clear_other_waypoints: bool,
) -> Result<(), ApiError> {
    use axum::http::StatusCode;
    let store = crate::db::PgTokenStore::new(state.db.clone());
    let token = state
        .auth
        .sso()
        .access_token(&store, character_id, crate::esi::Scope::WriteWaypoint)
        .await
        .map_err(|err| match err {
            crate::esi::EsiError::MissingScope(_) => ApiError {
                status: StatusCode::CONFLICT,
                message: "that character has not granted the waypoint scope".into(),
            },
            other => ApiError {
                status: StatusCode::BAD_GATEWAY,
                message: other.to_string(),
            },
        })?;
    state
        .auth
        .esi()
        .set_waypoint(
            &token,
            destination_id,
            add_to_beginning,
            clear_other_waypoints,
        )
        .await
        .map_err(|err| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: err.to_string(),
        })
}

/// ESI accepts a solar system, station, or structure id as a waypoint destination.
/// We can only vouch for the first two (structures are per-character ACL'd and not
/// seeded), so unknown ids are passed through to ESI rather than rejected here.
async fn validate_waypoint_destination(
    state: &AppState,
    destination_id: i64,
) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar!(
        "select exists(select 1 from solar_systems where id = $1)
             or exists(select 1 from stations where id = $1)",
        destination_id,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);
    if exists {
        Ok(())
    } else {
        Err(ApiError::bad_request("unknown destination"))
    }
}

/// `POST /api/waypoints` — set a destination/waypoint for one of the caller's characters.
pub async fn set_waypoint(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SetWaypointBody>,
) -> ApiResult<()> {
    let actor = require_actor(&state.db, &jar).await?;
    let owns = sqlx::query_scalar!(
        "select exists(select 1 from characters where id = $1 and user_id = $2)",
        body.character_id,
        actor.user_id,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);
    if !owns {
        return Err(ApiError::bad_request("that character isn't yours"));
    }
    validate_waypoint_destination(&state, body.destination_id).await?;
    esi_waypoint(
        &state,
        body.character_id,
        body.destination_id,
        body.add_to_beginning.unwrap_or(false),
        body.clear_other_waypoints.unwrap_or(true),
    )
    .await?;
    Ok(Json(()))
}

/// `POST /api/waypoints/all` — set the destination for every online character of the
/// caller. Best-effort: characters without the scope are skipped; fails only when none
/// succeed.
pub async fn set_waypoint_all(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SetWaypointAllBody>,
) -> ApiResult<()> {
    let actor = require_actor(&state.db, &jar).await?;
    validate_waypoint_destination(&state, body.destination_id).await?;
    let ids = sqlx::query_scalar!(
        "select c.id from characters c
         join character_status s on s.character_id = c.id and s.online
         where c.user_id = $1",
        actor.user_id,
    )
    .fetch_all(&state.db)
    .await?;
    if ids.is_empty() {
        return Err(ApiError::bad_request("no characters online"));
    }
    let mut ok = 0usize;
    let mut last_err = None;
    for id in ids {
        match esi_waypoint(
            &state,
            id,
            body.destination_id,
            body.add_to_beginning.unwrap_or(false),
            body.clear_other_waypoints.unwrap_or(true),
        )
        .await
        {
            Ok(()) => ok += 1,
            Err(err) => last_err = Some(err),
        }
    }
    if ok == 0 {
        return Err(last_err.expect("at least one attempt"));
    }
    Ok(Json(()))
}

// --- Config / reference data ---

/// `GET /api/grid-config` — the server-owned map canvas geometry.
pub async fn grid_config(State(state): State<AppState>) -> ApiResult<GridConfig> {
    Ok(Json(state.grid))
}

#[derive(Deserialize)]
pub struct EffectsQuery {
    pub name: String,
    pub class: i32,
}

/// `GET /api/effects?name=&class=` — the buffs/debuffs a wormhole effect applies at a
/// system's class. Reference data, so no actor/role check.
pub async fn effect_modifiers(
    State(state): State<AppState>,
    Query(query): Query<EffectsQuery>,
) -> ApiResult<Vec<EffectModifier>> {
    let mods =
        crate::maps::solar_system::effect_modifiers(&state.db, &query.name, query.class).await?;
    Ok(Json(mods))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

/// Build the sovereignty holder from the joined columns. Shared by every query that selects
/// a solar system for display, so the pickers cannot drift apart on what they show.
fn sovereignty_of(
    kind: Option<&str>,
    id: Option<i64>,
    name: Option<String>,
    ticker: Option<String>,
) -> Option<crate::maps::solar_system::Sovereignty> {
    match (kind, id, name) {
        (Some("alliance"), Some(id), Some(name)) => {
            Some(crate::maps::solar_system::Sovereignty::Alliance {
                id,
                name,
                ticker: ticker.unwrap_or_default(),
            })
        }
        (Some("corporation"), Some(id), Some(name)) => {
            Some(crate::maps::solar_system::Sovereignty::Corporation {
                id,
                name,
                ticker: ticker.unwrap_or_default(),
            })
        }
        (Some("faction"), Some(id), Some(name)) => {
            Some(crate::maps::solar_system::Sovereignty::Faction { id, name })
        }
        _ => None,
    }
}

/// `GET /api/systems/search?q=` — search the SDE solar systems by name. Prefix matches rank
/// first, then shorter names, then alphabetical. Returns nothing for queries under 2 chars.
pub async fn search_systems(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Vec<SystemSearchResult>> {
    require_actor(&state.db, &jar).await?;
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");
    let rows = sqlx::query!(
        r#"
        select s.id,
               s.name,
               s.security_status as "security!",
               r.name            as "region!",
               s.region_id,
               s.constellation_id,
               s.wormhole_class_id,
               ws.effect_name,
               case
                   when sov.alliance_id is not null then 'alliance'
                   when sov.corporation_id is not null then 'corporation'
                   when sov.faction_id is not null then 'faction'
               end as "sov_kind?",
               coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as "sov_id?",
               coalesce(al.name, co.name, f.name) as "sov_name?",
               coalesce(al.ticker, co.ticker) as "sov_ticker?"
        from solar_systems s
        join regions r on r.id = s.region_id
        left join wormhole_systems ws on ws.solar_system_id = s.id
        left join system_sovereignty sov on sov.solar_system_id = s.id
        left join alliances al on al.id = sov.alliance_id
        left join corporations co on co.id = sov.corporation_id
        left join factions f on f.id = sov.faction_id
        where s.name ilike $1
        order by (s.name ilike $2) desc, length(s.name), s.name
        limit 30
        "#,
        contains,
        prefix,
    )
    .fetch_all(&state.db)
    .await?;
    let results = rows
        .into_iter()
        .map(|row| {
            let sovereignty = sovereignty_of(
                row.sov_kind.as_deref(),
                row.sov_id,
                row.sov_name,
                row.sov_ticker,
            );
            SystemSearchResult {
                id: row.id,
                name: row.name,
                security: row.security,
                region: row.region,
                region_id: row.region_id,
                constellation_id: row.constellation_id,
                wormhole_class_id: row.wormhole_class_id,
                effect_name: row.effect_name,
                sovereignty,
            }
        })
        .collect();
    Ok(Json(results))
}

/// `GET /api/routing-graph` — the static half of the routing graph: k-space stargate
/// adjacency (`{from_id: [to_id, ...]}`, Zarzakh excluded since its gates are
/// faction-gated) plus per-system security for the safer/less-secure cost functions.
/// Static reference data: cacheable for a day.
pub async fn routing_graph(
    State(state): State<AppState>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    const ZARZAKH: i64 = 30100000;
    let rows = sqlx::query!(
        "select solar_system_id, destination_system_id from stargates
         where solar_system_id <> $1 and destination_system_id <> $1",
        ZARZAKH,
    )
    .fetch_all(&state.db)
    .await?;
    let mut adjacency: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for r in rows {
        adjacency
            .entry(r.solar_system_id)
            .or_default()
            .push(r.destination_system_id);
    }
    let security: std::collections::HashMap<i64, f64> =
        sqlx::query!("select id, security_status from solar_systems")
            .fetch_all(&state.db)
            .await?
            .into_iter()
            .map(|r| (r.id, r.security_status))
            .collect();
    let jove: Vec<i64> = sqlx::query_scalar!("select solar_system_id from jove_observatories")
        .fetch_all(&state.db)
        .await?;
    let stations: Vec<i64> = sqlx::query_scalar!("select distinct solar_system_id from stations")
        .fetch_all(&state.db)
        .await?;
    // The Find conditions offer the legacy "essential" station services. Each entry
    // carries the concrete stations, so results can name the station, not just the
    // system. Security Offices (27) are a known quirk: only CONCORD-owned stations in
    // lowsec actually run one, despite the operation listing the service everywhere.
    const ESSENTIAL_SERVICES: [i64; 6] = [5, 10, 13, 14, 15, 27];
    const SECURITY_OFFICE: i64 = 27;
    const CONCORD_CORPORATION: i64 = 1000125;
    let rows = sqlx::query!(
        r#"select ss.id as "service_id", ss.name as "service_name",
                  st.id as "station_id", st.name as "station_name",
                  st.solar_system_id
           from station_services ss
           join station_operation_services sos on sos.service_id = ss.id
           join stations st on st.operation_id = sos.operation_id
           join solar_systems sys on sys.id = st.solar_system_id
           where ss.id = any($1)
             and (ss.id <> $2
                  or (st.owner_corporation_id = $3
                      and sys.security_status > 0 and sys.security_status < 0.45))
           order by ss.name, st.name"#,
        &ESSENTIAL_SERVICES,
        SECURITY_OFFICE,
        CONCORD_CORPORATION,
    )
    .fetch_all(&state.db)
    .await?;
    let mut services: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<(i64, String, Vec<serde_json::Value>)> = None;
    for row in rows {
        let station = serde_json::json!({
            "id": row.station_id,
            "name": row.station_name,
            "solar_system_id": row.solar_system_id,
        });
        match &mut current {
            Some((id, _, stations)) if *id == row.service_id => stations.push(station),
            _ => {
                if let Some((id, name, stations)) = current.take() {
                    services
                        .push(serde_json::json!({ "id": id, "name": name, "stations": stations }));
                }
                current = Some((row.service_id, row.service_name, vec![station]));
            }
        }
    }
    if let Some((id, name, stations)) = current {
        services.push(serde_json::json!({ "id": id, "name": name, "stations": stations }));
    }
    Ok((
        [(axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
        Json(serde_json::json!({
            "adjacency": adjacency,
            "security": security,
            "jove": jove,
            "stations": stations,
            "services": services,
        })),
    ))
}

/// `GET /api/systems/resolve?ids=a,b,c` — resolve solar system ids to display data for
/// route rows. Capped at 200 ids.
pub async fn resolve_systems(
    State(state): State<AppState>,
    Query(query): Query<ResolveQuery>,
) -> ApiResult<Vec<SystemSearchResult>> {
    let ids: Vec<i64> = query
        .ids
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .take(200)
        .collect();
    let rows = sqlx::query!(
        r#"
        select s.id,
               s.name,
               s.security_status as "security!",
               r.name            as "region!",
               s.region_id,
               s.constellation_id,
               s.wormhole_class_id,
               ws.effect_name,
               case
                   when sov.alliance_id is not null then 'alliance'
                   when sov.corporation_id is not null then 'corporation'
                   when sov.faction_id is not null then 'faction'
               end as "sov_kind?",
               coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as "sov_id?",
               coalesce(al.name, co.name, f.name) as "sov_name?",
               coalesce(al.ticker, co.ticker) as "sov_ticker?"
        from solar_systems s
        join regions r on r.id = s.region_id
        left join wormhole_systems ws on ws.solar_system_id = s.id
        left join system_sovereignty sov on sov.solar_system_id = s.id
        left join alliances al on al.id = sov.alliance_id
        left join corporations co on co.id = sov.corporation_id
        left join factions f on f.id = sov.faction_id
        where s.id = any($1)"#,
        &ids,
    )
    .fetch_all(&state.db)
    .await?;
    let results = rows
        .into_iter()
        .map(|row| {
            let sovereignty = match (row.sov_kind.as_deref(), row.sov_id, row.sov_name) {
                (Some("alliance"), Some(id), Some(name)) => {
                    Some(crate::maps::solar_system::Sovereignty::Alliance {
                        id,
                        name,
                        ticker: row.sov_ticker.unwrap_or_default(),
                    })
                }
                (Some("corporation"), Some(id), Some(name)) => {
                    Some(crate::maps::solar_system::Sovereignty::Corporation {
                        id,
                        name,
                        ticker: row.sov_ticker.unwrap_or_default(),
                    })
                }
                (Some("faction"), Some(id), Some(name)) => {
                    Some(crate::maps::solar_system::Sovereignty::Faction { id, name })
                }
                _ => None,
            };
            SystemSearchResult {
                id: row.id,
                name: row.name,
                security: row.security,
                region: row.region,
                region_id: row.region_id,
                constellation_id: row.constellation_id,
                wormhole_class_id: row.wormhole_class_id,
                effect_name: row.effect_name,
                sovereignty,
            }
        })
        .collect();
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct ResolveQuery {
    pub ids: String,
}

/// `GET /api/threat/{solar_system_id}` — a wormhole system's threat analysis. 404 for
/// k-space systems (threat is only computed for wormhole space).
pub async fn threat_analysis(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(solar_system_id): Path<i64>,
) -> ApiResult<ThreatAnalysis> {
    require_actor(&state.db, &jar).await?;
    let row = sqlx::query!(
        r#"select threat_level as "threat_level: crate::maps::ThreatLevel", threat_analyzed_at
           from wormhole_systems where solar_system_id = $1"#,
        solar_system_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(crate::maps::MapError::NotFound)?;
    let entities = sqlx::query!(
        "select entity_id, entity_type, name, kills from wormhole_system_threats
         where solar_system_id = $1 order by kills desc",
        solar_system_id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(ThreatAnalysis {
        threat_level: row.threat_level,
        threat_analyzed_at: row.threat_analyzed_at,
        entities: entities
            .into_iter()
            .map(|e| ThreatEntity {
                id: e.entity_id,
                name: e.name,
                entity_type: e.entity_type,
                kills: e.kills,
            })
            .collect(),
    }))
}

// --- Maps ---

/// `GET /api/maps` — every map the signed-in character can access, with their role.
pub async fn my_maps(State(state): State<AppState>, jar: CookieJar) -> ApiResult<Vec<MapEntry>> {
    let actor = require_actor(&state.db, &jar).await?;
    let maps = crate::maps::map::list_maps(&state.db, actor.user_id).await?;
    Ok(Json(
        maps.into_iter()
            .map(|(m, role)| MapEntry {
                id: m.id,
                name: m.name,
                role: role.as_str().to_string(),
            })
            .collect(),
    ))
}

#[derive(Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CreateMapBody {
    pub name: String,
}

/// `POST /api/maps` — create a map owned by the active character.
pub async fn create_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<CreateMapBody>,
) -> ApiResult<Map> {
    let actor = require_actor(&state.db, &jar).await?;
    let map = crate::maps::map::create_map(
        &state.db,
        actor,
        crate::maps::map::CreateMap {
            name: body.name,
            description: None,
        },
    )
    .await?;
    Ok(Json(map))
}

/// `DELETE /api/maps/{id}` — delete a map (owner only).
pub async fn delete_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<()> {
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::map::delete_map(&state.db, actor, crate::maps::map::DeleteMap { map_id }).await?;
    Ok(Json(()))
}

/// `GET /api/maps/{id}` — the full map view (map + systems + connections).
pub async fn fetch_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<MapView> {
    let actor = require_actor(&state.db, &jar).await?;
    let view =
        crate::maps::map::get_map(&state.db, actor, crate::maps::map::GetMap { map_id }).await?;
    Ok(Json(view))
}

/// `GET /api/maps/{id}/signatures` — all signatures on the map.
pub async fn list_signatures(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<Signature>> {
    let actor = require_actor(&state.db, &jar).await?;
    let sigs = crate::maps::signatures::list_signatures(&state.db, actor, map_id).await?;
    Ok(Json(sigs))
}

/// `GET /api/maps/{id}/settings/user` — the caller's per-map preferences (defaults when
/// no row exists yet). Requires any access to the map.
pub async fn map_user_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<MapUserSettings> {
    let actor = require_actor(&state.db, &jar).await?;
    if crate::maps::access::effective_role(&state.db, map_id, actor.user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::from(crate::maps::MapError::NotFound));
    }
    let row = sqlx::query!(
        "select tracking_allowed, show_threat_level, compact_signature_list, show_statics_first,
                route_preference, security_penalty, route_allow_time_status,
                route_allow_mass_status, route_use_evescout, prompt_for_signature,
                suggest_alias, copy_bookmark, hidden_panels, layout_breakpoints
         from map_user_settings where map_id = $1 and user_id = $2",
        map_id,
        actor.user_id,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(match row {
        Some(r) => MapUserSettings {
            tracking_allowed: r.tracking_allowed,
            show_threat_level: r.show_threat_level,
            compact_signature_list: r.compact_signature_list,
            show_statics_first: r.show_statics_first,
            route_preference: r.route_preference,
            security_penalty: r.security_penalty,
            route_allow_time_status: r.route_allow_time_status,
            route_allow_mass_status: r.route_allow_mass_status,
            route_use_evescout: r.route_use_evescout,
            prompt_for_signature: r.prompt_for_signature,
            suggest_alias: r.suggest_alias,
            copy_bookmark: r.copy_bookmark,
            hidden_panels: r.hidden_panels,
            layout_breakpoints: r
                .layout_breakpoints
                .map(serde_json::from_value)
                .transpose()
                .unwrap_or(None),
        },
        None => MapUserSettings {
            tracking_allowed: false,
            show_threat_level: true,
            compact_signature_list: false,
            show_statics_first: false,
            route_preference: "shorter".into(),
            security_penalty: 50,
            route_allow_time_status: "critical".into(),
            route_allow_mass_status: "reduced".into(),
            route_use_evescout: false,
            prompt_for_signature: true,
            suggest_alias: true,
            copy_bookmark: false,
            hidden_panels: Vec::new(),
            layout_breakpoints: None,
        },
    }))
}

/// `POST /api/maps/{id}/settings/user` — partial update (upsert) of the caller's per-map
/// preferences.
pub async fn update_map_user_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(body): Json<UpdateMapUserSettings>,
) -> ApiResult<MapUserSettings> {
    let actor = require_actor(&state.db, &jar).await?;
    if crate::maps::access::effective_role(&state.db, map_id, actor.user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::from(crate::maps::MapError::NotFound));
    }
    if let Some(p) = body.route_preference.as_deref()
        && !matches!(p, "shorter" | "safer" | "less_secure")
    {
        return Err(ApiError::bad_request("invalid route preference"));
    }
    if let Some(p) = body.security_penalty
        && !(0..=100).contains(&p)
    {
        return Err(ApiError::bad_request("security penalty must be 0-100"));
    }
    if let Some(t) = body.route_allow_time_status.as_deref()
        && !matches!(t, "stable" | "eol" | "critical")
    {
        return Err(ApiError::bad_request("invalid lifetime tolerance"));
    }
    if let Some(m) = body.route_allow_mass_status.as_deref()
        && !matches!(m, "stable" | "reduced" | "critical")
    {
        return Err(ApiError::bad_request("invalid mass tolerance"));
    }
    // Reject an arrangement that could not render, rather than storing it and breaking
    // the page on the next load.
    let layout_json =
        match &body.layout_breakpoints {
            Some(layouts) => {
                super::validate_layouts(layouts)?;
                Some(serde_json::to_value(layouts).map_err(|e| {
                    ApiError::bad_request(format!("could not store the layout: {e}"))
                })?)
            }
            None => None,
        };

    let row = sqlx::query!(
        "insert into map_user_settings
             (map_id, user_id, tracking_allowed, show_threat_level,
              compact_signature_list, show_statics_first,
              route_preference, security_penalty, route_allow_time_status,
              route_allow_mass_status, route_use_evescout, prompt_for_signature,
              suggest_alias, copy_bookmark, hidden_panels, layout_breakpoints)
         values ($1, $2, coalesce($3, false), coalesce($4, true),
                 coalesce($5, false), coalesce($6, false),
                 coalesce($7, 'shorter'), coalesce($8, 50), coalesce($9, 'critical'),
                 coalesce($10, 'reduced'), coalesce($11, false),
                 coalesce($12, true), coalesce($13, true), coalesce($14, false),
                 coalesce($15, '{}'::text[]), $16)
         on conflict (map_id, user_id) do update set
             tracking_allowed = coalesce($3, map_user_settings.tracking_allowed),
             show_threat_level = coalesce($4, map_user_settings.show_threat_level),
             compact_signature_list = coalesce($5, map_user_settings.compact_signature_list),
             show_statics_first = coalesce($6, map_user_settings.show_statics_first),
             route_preference = coalesce($7, map_user_settings.route_preference),
             security_penalty = coalesce($8, map_user_settings.security_penalty),
             route_allow_time_status = coalesce($9, map_user_settings.route_allow_time_status),
             route_allow_mass_status = coalesce($10, map_user_settings.route_allow_mass_status),
             route_use_evescout = coalesce($11, map_user_settings.route_use_evescout),
             prompt_for_signature = coalesce($12, map_user_settings.prompt_for_signature),
             suggest_alias = coalesce($13, map_user_settings.suggest_alias),
             copy_bookmark = coalesce($14, map_user_settings.copy_bookmark),
             hidden_panels = coalesce($15, map_user_settings.hidden_panels),
             layout_breakpoints = coalesce($16, map_user_settings.layout_breakpoints),
             updated_at = now()
         returning tracking_allowed, show_threat_level, compact_signature_list,
                   show_statics_first, route_preference, security_penalty,
                   route_allow_time_status, route_allow_mass_status, route_use_evescout,
                   prompt_for_signature, suggest_alias, copy_bookmark,
                   hidden_panels, layout_breakpoints",
        map_id,
        actor.user_id,
        body.tracking_allowed,
        body.show_threat_level,
        body.compact_signature_list,
        body.show_statics_first,
        body.route_preference,
        body.security_penalty,
        body.route_allow_time_status,
        body.route_allow_mass_status,
        body.route_use_evescout,
        body.prompt_for_signature,
        body.suggest_alias,
        body.copy_bookmark,
        body.hidden_panels.as_deref(),
        layout_json.as_ref(),
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(MapUserSettings {
        tracking_allowed: row.tracking_allowed,
        show_threat_level: row.show_threat_level,
        compact_signature_list: row.compact_signature_list,
        show_statics_first: row.show_statics_first,
        route_preference: row.route_preference,
        security_penalty: row.security_penalty,
        route_allow_time_status: row.route_allow_time_status,
        route_allow_mass_status: row.route_allow_mass_status,
        route_use_evescout: row.route_use_evescout,
        prompt_for_signature: row.prompt_for_signature,
        suggest_alias: row.suggest_alias,
        copy_bookmark: row.copy_bookmark,
        hidden_panels: row.hidden_panels,
        layout_breakpoints: row
            .layout_breakpoints
            .map(serde_json::from_value)
            .transpose()
            .unwrap_or(None),
    }))
}

/// `GET /api/maps/{id}/characters` — presence: online characters of users who opted into
/// tracking on this map, holding the location scope, whose user has member-or-better
/// access. Member+ may view (viewers never see pilot data).
pub async fn map_characters(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<MapCharacter>> {
    let actor = require_actor(&state.db, &jar).await?;
    match crate::maps::access::effective_role(&state.db, map_id, actor.user_id).await? {
        None => return Err(ApiError::from(crate::maps::MapError::NotFound)),
        Some(crate::maps::Role::Viewer) => {
            return Err(ApiError::from(crate::maps::MapError::Forbidden));
        }
        Some(_) => {}
    }
    let rows = sqlx::query!(
        r#"select c.id as character_id, c.name, co.ticker as corporation_ticker,
                  s.solar_system_id, s.ship_type_id, s.ship_name, t.name as "ship_type?"
           from characters c
           join users u on u.id = c.user_id
           join map_user_settings mus
               on mus.map_id = $1 and mus.user_id = u.id and mus.tracking_allowed
           join character_status s on s.character_id = c.id and s.online
           join corporations co on co.id = c.corporation_id
           left join types t on t.id = s.ship_type_id
           where exists (
                     select 1 from esi_tokens et
                     join esi_token_scopes ets on ets.token_id = et.id
                     join esi_scopes sc on sc.id = ets.scope_id
                     where et.character_id = c.id
                       and sc.name = 'esi-location.read_location.v1'
                 )
             and exists (
                     select 1 from map_access ma
                     where ma.map_id = $1
                       and ma.role <> 'viewer'
                       and ma.subject_id in (
                           select id from characters where user_id = u.id
                           union all
                           select corporation_id from characters where user_id = u.id
                           union all
                           select alliance_id from characters
                           where user_id = u.id and alliance_id is not null
                       )
                 )
           order by c.name"#,
        map_id,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| MapCharacter {
                character_id: r.character_id,
                name: r.name,
                corporation_ticker: r.corporation_ticker,
                solar_system_id: r.solar_system_id,
                ship_type_id: r.ship_type_id,
                ship_name: r.ship_name,
                ship_type: r.ship_type,
            })
            .collect(),
    ))
}

// --- Systems ---

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

// Bulk removal (multi-select delete + clear map). Publishes a single coarse `MapUpdated`
// so each viewer does one full refetch rather than reacting to a storm of per-system events.

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

// --- System details (alias / status / occupier / home / rally / pinned) ---
//
// All publish `SystemDetailsChanged` (position unchanged; details refetched).

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

/// `GET /api/maps/{id}/systems/{mss}/details` — member-gated intel (notes). 403 for viewers.
pub async fn system_details(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, mss)): Path<(i64, i64)>,
) -> ApiResult<SystemDetails> {
    let actor = require_actor(&state.db, &jar).await?;
    let details = crate::maps::solar_system::system_details(&state.db, actor, map_id, mss).await?;
    Ok(Json(details))
}

// --- Connections ---

pub async fn add_connection(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddConnection>,
) -> ApiResult<MapConnection> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
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

// --- Signatures ---

pub async fn add_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddSignature>,
) -> ApiResult<Signature> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let sig = crate::maps::signatures::add_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(Json(sig))
}

pub async fn paste_signatures(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<PasteSignatures>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let solar_system_id = cmd.solar_system_id;
    crate::maps::signatures::paste_signatures(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id,
        solar_system_id,
    });
    Ok(Json(()))
}

pub async fn update_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UpdateSignature>,
) -> ApiResult<Signature> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let sig = crate::maps::signatures::update_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    if let Some(connection_id) = sig.connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id: sig.map_id,
            connection_id,
        });
    }
    Ok(Json(sig))
}

pub async fn link_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<LinkSignature>,
) -> ApiResult<Signature> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let connection_id = cmd.connection_id;
    let sig = crate::maps::signatures::link_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    state.hub.publish(MapEvent::ConnectionChanged {
        map_id: sig.map_id,
        connection_id,
    });
    Ok(Json(sig))
}

pub async fn unlink_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UnlinkSignature>,
) -> ApiResult<Signature> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let sig = crate::maps::signatures::unlink_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(Json(sig))
}

pub async fn remove_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSignature>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let outcome = crate::maps::signatures::remove_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id,
        solar_system_id: outcome.solar_system_id,
    });
    if let Some(connection_id) = outcome.removed_connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    Ok(Json(()))
}

/// `POST /api/maps/{id}/signatures/remove-bulk` — the panel's "delete missing
/// signatures" path, with the legacy connection + orphan-endpoint cascade.
pub async fn remove_signatures_bulk(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSignatures>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let outcome = crate::maps::signatures::remove_signatures(&state.db, actor, cmd).await?;
    for solar_system_id in outcome.systems {
        state.hub.publish(MapEvent::SignatureChanged {
            map_id,
            solar_system_id,
        });
    }
    for connection_id in outcome.removed_connection_ids {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    for map_solar_system_id in outcome.removed_placement_ids {
        state.hub.publish(MapEvent::SystemRemoved {
            map_id,
            map_solar_system_id,
        });
    }
    Ok(Json(()))
}

/// `GET /api/signature-types` — the seeded signature catalog (categories + types with
/// spawn areas). Reference data, so no actor/role check; cacheable for a day.
pub async fn signature_catalog(
    State(state): State<AppState>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let categories = sqlx::query_as!(
        SignatureCategoryInfo,
        "select id, name, code from signature_categories order by id",
    )
    .fetch_all(&state.db)
    .await?;
    let types = sqlx::query!(
        r#"select st.id, st.signature, st.name, st.signature_category_id, st.target_class,
                  st.extra,
                  coalesce(array_agg(sa.wormhole_class_id order by sa.wormhole_class_id)
                           filter (where sa.wormhole_class_id is not null), '{}') as "spawn_areas!",
                  wt.total_mass, wt.max_mass_per_jump, wt.lifetime_hours, wt.signature_strength
           from signature_types st
           left join wormhole_types wt on wt.code = st.signature
           left join signature_type_spawn_areas sa on sa.signature_type_id = st.id
           group by st.id, wt.code
           order by st.id"#,
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| SignatureTypeInfo {
        id: r.id,
        signature: r.signature,
        name: r.name,
        signature_category_id: r.signature_category_id,
        target_class: r.target_class,
        extra: r.extra,
        spawn_areas: r.spawn_areas,
        total_mass: r.total_mass,
        max_jump_mass: r.max_mass_per_jump,
        lifetime_hours: r.lifetime_hours,
        signature_strength: r.signature_strength,
    })
    .collect();
    Ok((
        [(axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
        Json(SignatureCatalog { categories, types }),
    ))
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

// --- Connection jumps ---

/// `GET /api/maps/{id}/connections/{cid}/jumps` — the latest 10 jump-log rows.
pub async fn list_connection_jumps(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, connection_id)): Path<(i64, i64)>,
) -> ApiResult<Vec<ConnectionJump>> {
    let actor = require_actor(&state.db, &jar).await?;
    let jumps = crate::maps::jumps::list_jumps(&state.db, actor, map_id, connection_id).await?;
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

// --- Watchlist ---

/// `GET /api/maps/{id}/watchlist` — the map's tracked destinations.
pub async fn list_watchlist(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<WatchlistEntry>> {
    let actor = require_actor(&state.db, &jar).await?;
    let entries = crate::maps::watchlist::list_watchlist(&state.db, actor, map_id).await?;
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

/// `POST /api/maps/{id}/update` — rename a map or change its description/image. Manager+.
pub async fn update_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UpdateMap>,
) -> ApiResult<Map> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    let map = crate::maps::map::update_map(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(map))
}

/// `GET /api/maps/{id}/search?q=` — the map command palette. Matches placed systems by
/// name, alias, occupier and (for members) notes, then falls back to off-map systems the
/// palette can offer to add. Viewer+.
pub async fn search_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Vec<MapSearchHit>> {
    let actor = require_actor(&state.db, &jar).await?;
    let role = crate::maps::access::effective_role(&state.db, map_id, actor.user_id)
        .await?
        .ok_or(crate::maps::MapError::NotFound)?;
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    // Notes are member-gated everywhere else, so they must not leak through search.
    let with_notes = role >= crate::maps::Role::Member;
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");

    // The same columns the standalone system search selects, so both build an identical
    // SystemSearchResult and the pickers render the same way.
    let placed = sqlx::query!(
        r#"select mss.id as "map_solar_system_id!", ss.id as "solar_system_id!",
                  ss.name as "name!", r.name as "region!",
                  ss.security_status as "security!", ss.region_id, ss.constellation_id,
                  ss.wormhole_class_id, ws.effect_name,
                  case
                      when sov.alliance_id is not null then 'alliance'
                      when sov.corporation_id is not null then 'corporation'
                      when sov.faction_id is not null then 'faction'
                  end as "sov_kind?",
                  coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as "sov_id?",
                  coalesce(al.name, co.name, f.name) as "sov_name?",
                  coalesce(al.ticker, co.ticker) as "sov_ticker?",
                  mss.alias, d.occupying_group, d.notes
           from map_solar_systems mss
           join solar_systems ss on ss.id = mss.solar_system_id
           join regions r on r.id = ss.region_id
           left join wormhole_systems ws on ws.solar_system_id = ss.id
           left join system_sovereignty sov on sov.solar_system_id = ss.id
           left join alliances al on al.id = sov.alliance_id
           left join corporations co on co.id = sov.corporation_id
           left join factions f on f.id = sov.faction_id
           left join map_solar_system_details d
             on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
           where mss.map_id = $1
             and (ss.name ilike $2 or mss.alias ilike $2 or d.occupying_group ilike $2
                  or ($4 and d.notes ilike $2))
           order by (ss.name ilike $3) desc, ss.name
           limit 25"#,
        map_id,
        contains,
        prefix,
        with_notes,
    )
    .fetch_all(&state.db)
    .await?;

    let lower = q.to_lowercase();
    let mut hits: Vec<MapSearchHit> = placed
        .into_iter()
        .map(|r| {
            let hit = |v: &Option<String>| {
                v.as_ref()
                    .is_some_and(|s| s.to_lowercase().contains(&lower))
            };
            let matched = if r.name.to_lowercase().contains(&lower) {
                "name"
            } else if hit(&r.alias) {
                "alias"
            } else if hit(&r.occupying_group) {
                "occupier"
            } else {
                "notes"
            };
            MapSearchHit {
                system: SystemSearchResult {
                    id: r.solar_system_id,
                    name: r.name,
                    security: r.security,
                    region: r.region,
                    region_id: r.region_id,
                    constellation_id: r.constellation_id,
                    wormhole_class_id: r.wormhole_class_id,
                    effect_name: r.effect_name,
                    sovereignty: sovereignty_of(
                        r.sov_kind.as_deref(),
                        r.sov_id,
                        r.sov_name,
                        r.sov_ticker,
                    ),
                },
                map_solar_system_id: Some(r.map_solar_system_id),
                alias: r.alias,
                occupying_group: r.occupying_group,
                note_excerpt: if matched == "notes" {
                    r.notes.map(|n| excerpt(&n, q))
                } else {
                    None
                },
                matched: matched.into(),
            }
        })
        .collect();

    // Then systems that are not on the map yet, so the palette doubles as "add".
    let placed_ids: Vec<i64> = hits.iter().map(|h| h.system.id).collect();
    let off_map = sqlx::query!(
        r#"select ss.id as "solar_system_id!", ss.name as "name!", r.name as "region!",
                  ss.security_status as "security!", ss.region_id, ss.constellation_id,
                  ss.wormhole_class_id, ws.effect_name,
                  case
                      when sov.alliance_id is not null then 'alliance'
                      when sov.corporation_id is not null then 'corporation'
                      when sov.faction_id is not null then 'faction'
                  end as "sov_kind?",
                  coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as "sov_id?",
                  coalesce(al.name, co.name, f.name) as "sov_name?",
                  coalesce(al.ticker, co.ticker) as "sov_ticker?"
           from solar_systems ss
           join regions r on r.id = ss.region_id
           left join wormhole_systems ws on ws.solar_system_id = ss.id
           left join system_sovereignty sov on sov.solar_system_id = ss.id
           left join alliances al on al.id = sov.alliance_id
           left join corporations co on co.id = sov.corporation_id
           left join factions f on f.id = sov.faction_id
           where ss.name ilike $1 and ss.id <> all($2)
           order by (ss.name ilike $3) desc, length(ss.name), ss.name
           limit 10"#,
        contains,
        &placed_ids,
        prefix,
    )
    .fetch_all(&state.db)
    .await?;
    hits.extend(off_map.into_iter().map(|r| MapSearchHit {
        system: SystemSearchResult {
            id: r.solar_system_id,
            name: r.name,
            security: r.security,
            region: r.region,
            region_id: r.region_id,
            constellation_id: r.constellation_id,
            wormhole_class_id: r.wormhole_class_id,
            effect_name: r.effect_name,
            sovereignty: sovereignty_of(r.sov_kind.as_deref(), r.sov_id, r.sov_name, r.sov_ticker),
        },
        map_solar_system_id: None,
        alias: None,
        occupying_group: None,
        note_excerpt: None,
        matched: "name".into(),
    }));
    Ok(Json(hits))
}

/// A one-line window of `notes` around the first match, so the palette shows why a note hit.
fn excerpt(notes: &str, needle: &str) -> String {
    const WINDOW: usize = 60;
    let at = notes
        .to_lowercase()
        .find(&needle.to_lowercase())
        .unwrap_or(0);
    let start = notes[..at]
        .char_indices()
        .rev()
        .take(WINDOW)
        .last()
        .map_or(at, |(i, _)| i);
    let end = notes[at..]
        .char_indices()
        .take(needle.len() + WINDOW)
        .last()
        .map_or(notes.len(), |(i, c)| at + i + c.len_utf8());
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(notes[start..end].trim());
    if end < notes.len() {
        out.push('…');
    }
    out.replace('\n', " ")
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

// --- Access ---

/// `GET /api/access-subjects/search?q=` — characters, corporations and alliances that can
/// be granted access. Only entities Vector has already cached are searchable (a character
/// who has signed in, or a corp/alliance one of them belongs to), which is why the UI also
/// accepts a raw EVE id.
pub async fn search_access_subjects(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Vec<AccessSubject>> {
    require_actor(&state.db, &jar).await?;
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");
    let rows = sqlx::query!(
        // The `!` overrides restate what the source tables guarantee: sqlx cannot see
        // through the union and widens every column to nullable.
        r#"select id as "id!", name as "name!",
                  kind as "kind!: crate::maps::SubjectType", ticker
           from (
               select id, name, 'character' as kind, null::text as ticker from characters
               union all
               select id, name, 'corporation', ticker from corporations
               union all
               select id, name, 'alliance', ticker from alliances
           ) s
           where s.name ilike $1 or s.ticker ilike $1
           order by (s.name ilike $2) desc, length(s.name), s.name
           limit 20"#,
        contains,
        prefix,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| AccessSubject {
                subject_type: r.kind,
                subject_id: r.id,
                name: r.name,
                ticker: r.ticker,
            })
            .collect(),
    ))
}

/// `GET /api/maps/{id}/access` — who can see this map, and at what role. Viewer+.
pub async fn list_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<AccessEntry>> {
    let actor = require_actor(&state.db, &jar).await?;
    let entries = crate::maps::access::list_access(&state.db, actor, map_id).await?;
    Ok(Json(entries))
}

pub async fn set_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<SetAccess>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::access::set_access(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::AccessChanged { map_id });
    Ok(Json(()))
}

pub async fn revoke_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RevokeAccess>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::access::revoke_access(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::AccessChanged { map_id });
    Ok(Json(()))
}

// --- History ---

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

/// `POST /api/maps/{id}/track-jump` — record a jump: place the system, connect it, and
/// link the signature it turned out to be. Member+. One command, so it undoes as one step;
/// it can touch a system, a connection and a signature at once, so clients just refetch.
pub async fn track_jump(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<crate::maps::tracking::TrackJump>,
) -> ApiResult<()> {
    check_map_id(map_id, cmd.map_id)?;
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::tracking::track_jump(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::HistoryChanged { map_id });
    Ok(Json(()))
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

// --- EVE Scout ---

/// Normalize one EVE Scout signature into a router edge. Tolerant of shape drift:
/// unknown fields default to healthy/fresh, missing endpoints drop the row.
pub(crate) fn eve_scout_edge(sig: &serde_json::Value) -> Option<EveScoutEdge> {
    let from = sig.get("in_system_id")?.as_i64()?;
    let to = sig.get("out_system_id")?.as_i64()?;
    let mass = match sig.get("mass").and_then(|v| v.as_str()).unwrap_or("") {
        m if m.contains("crit") => "critical",
        m if m.contains("reduced") || m.contains("destab") => "reduced",
        _ => "stable",
    };
    let time = match sig.get("life").and_then(|v| v.as_str()) {
        Some(l) if l.contains("crit") => "critical",
        Some(l) if l.contains("eol") => "eol",
        Some(_) => "stable",
        // No life field: derive from the remaining lifetime when present.
        None => match sig.get("remaining_hours").and_then(|v| v.as_f64()) {
            Some(h) if h < 1.0 => "critical",
            Some(h) if h < 4.0 => "eol",
            _ => "stable",
        },
    };
    Some(EveScoutEdge {
        from_solar_system_id: from,
        to_solar_system_id: to,
        mass_status: mass.into(),
        time_status: time.into(),
    })
}

/// `GET /api/evescout` — public Thera/Turnur connections, proxied and cached for 60s.
/// Upstream failures degrade to an empty list.
pub async fn eve_scout(State(_state): State<AppState>) -> ApiResult<Vec<EveScoutEdge>> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Cached = Option<(Instant, Vec<EveScoutEdge>)>;
    static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    if let Some((at, edges)) = cache.lock().expect("cache lock").as_ref()
        && at.elapsed() < Duration::from_secs(60)
    {
        return Ok(Json(edges.clone()));
    }

    let edges = fetch_eve_scout().await.unwrap_or_default();
    *cache.lock().expect("cache lock") = Some((Instant::now(), edges.clone()));
    Ok(Json(edges))
}

async fn fetch_eve_scout() -> Option<Vec<EveScoutEdge>> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "vector-wormhole-mapper/",
            env!("CARGO_PKG_VERSION"),
            " (tim.kunze4@gmail.com)"
        ))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get("https://api.eve-scout.com/v2/public/signatures")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    Some(body.as_array()?.iter().filter_map(eve_scout_edge).collect())
}

#[cfg(test)]
mod eve_scout_tests {
    use super::eve_scout_edge;

    #[test]
    fn normalizes_statuses_and_drops_incomplete_rows() {
        let sig = serde_json::json!({
            "in_system_id": 30000142, "out_system_id": 31000005,
            "mass": "destab", "life": "eol"
        });
        let edge = eve_scout_edge(&sig).unwrap();
        assert_eq!(edge.mass_status, "reduced");
        assert_eq!(edge.time_status, "eol");

        let sig = serde_json::json!({
            "in_system_id": 30000142, "out_system_id": 31000005,
            "remaining_hours": 0.5
        });
        let edge = eve_scout_edge(&sig).unwrap();
        assert_eq!(edge.mass_status, "stable");
        assert_eq!(edge.time_status, "critical");

        let sig = serde_json::json!({ "in_system_id": 30000142 });
        assert!(eve_scout_edge(&sig).is_none());
    }
}
