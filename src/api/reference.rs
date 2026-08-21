//! Reference reads: the fixed universe and the caches over it. None of it belongs to a
//! map, so none of it is authorized against one; a few need a session, no more.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::extract::require_actor;
use super::{ApiError, ApiResult};
use crate::auth::AppState;
use crate::maps::{EffectModifier, GridConfig};

/// A solar system matched by the "add system" search, with just enough to display and pick.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SystemSearchResult {
    pub id: i64,
    pub name: String,
    pub security: f64,
    pub region: String,
    pub region_id: i64,
    pub constellation_id: i64,
    pub wormhole_class_id: Option<i32>,
    /// Wormhole effect, for J-space rows (shown where k-space rows show sovereignty).
    pub effect_name: Option<String>,
    pub sovereignty: Option<crate::maps::solar_system::Sovereignty>,
    /// The statics a wormhole always has. Empty for k-space.
    pub statics: Vec<crate::maps::solar_system::Static>,
}

/// One organisation in a system's threat top list.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ThreatEntity {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub kills: i32,
}

/// A wormhole system's threat analysis, for the threat card.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ThreatAnalysis {
    pub threat_level: crate::maps::ThreatLevel,
    pub threat_analyzed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub entities: Vec<ThreatEntity>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/instance", get(instance))
        .route("/api/grid-config", get(grid_config))
        .route("/api/effects", get(effect_modifiers))
        .route("/api/systems/search", get(search_systems))
        .route("/api/systems/resolve", get(resolve_systems))
        .route("/api/routing-graph", get(routing_graph))
        .route("/api/threat/{solar_system_id}", get(threat_analysis))
        .route("/api/server-status", get(server_status))
        .route("/api/skyhooks", get(skyhooks))
        .route("/api/reference-counts", get(reference_counts))
}

/// How much of New Eden this install has loaded. The landing page states these, so they
/// are the real contents of this database rather than numbers written into a page.
#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ReferenceCounts {
    pub solar_systems: i64,
    pub wormhole_systems: i64,
    pub stargates: i64,
    pub wormhole_types: i64,
}

/// `GET /api/reference-counts`: how much static data is seeded. Changes only when a new
/// SDE build is loaded.
pub async fn reference_counts(State(state): State<AppState>) -> ApiResult<ReferenceCounts> {
    let row = sqlx::query!(
        r#"select
             (select count(*) from solar_systems) as "solar_systems!",
             (select count(*) from wormhole_systems) as "wormhole_systems!",
             (select count(*) from stargates) as "stargates!",
             (select count(*) from wormhole_types) as "wormhole_types!""#
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(ReferenceCounts {
        solar_systems: row.solar_systems,
        wormhole_systems: row.wormhole_systems,
        stargates: row.stargates,
        wormhole_types: row.wormhole_types,
    }))
}

/// What this deployment can actually do, so the interface can say what is switched off
/// rather than offering something that will quietly never arrive. Self-hosters configure
/// Discord separately from the rest, and most of them do not configure it at all.
#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Instance {
    pub discord: DiscordCapability,
}

#[derive(Serialize, ts_rs::TS)]
#[ts(export)]
pub struct DiscordCapability {
    /// Whether an account can be linked at all: the OAuth half of the application.
    pub linking: bool,
    /// Whether the bot can post to a channel or DM. Needs a bot token on top of the rest;
    /// a plain webhook destination works without it.
    pub bot: bool,
}

/// `GET /api/instance`: what this deployment has switched on. No actor check; it says
/// nothing an anonymous visitor could not infer from which buttons appear.
pub async fn instance(State(state): State<AppState>) -> ApiResult<Instance> {
    Ok(Json(Instance {
        discord: DiscordCapability {
            linking: state.discord.is_some(),
            bot: state
                .discord
                .as_ref()
                .is_some_and(|d| d.bot_token.is_some()),
        },
    }))
}

/// `GET /api/grid-config`: the server-owned map canvas geometry.
pub async fn grid_config(State(state): State<AppState>) -> ApiResult<GridConfig> {
    Ok(Json(state.grid))
}

#[derive(Deserialize)]
pub struct EffectsQuery {
    pub name: String,
    pub class: i32,
}

/// `GET /api/effects?name=&class=`: the buffs/debuffs a wormhole effect applies at a
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

/// Build the sovereignty holder from the joined columns.
pub(super) fn sovereignty_of(
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

/// The statics of every wormhole among `ids`, grouped by system. Kept out of the search
/// query itself: statics are one-to-many, so joining would multiply its ranked rows.
pub(super) async fn statics_for(
    db: &sqlx::PgPool,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, Vec<crate::maps::solar_system::Static>>, ApiError> {
    use crate::maps::solar_system::Static;
    let mut out: std::collections::HashMap<i64, Vec<Static>> = std::collections::HashMap::new();
    if ids.is_empty() {
        return Ok(out);
    }
    let rows = sqlx::query!(
        "select wss.solar_system_id, wt.code, wt.dest_class,
                wt.total_mass, wt.max_mass_per_jump, wt.lifetime_hours, wt.signature_strength
         from wormhole_system_statics wss
         join wormhole_types wt on wt.code = wss.wormhole_code
         where wss.solar_system_id = any($1)
         order by wt.dest_class nulls last, wt.code",
        ids,
    )
    .fetch_all(db)
    .await?;
    for row in rows {
        out.entry(row.solar_system_id).or_default().push(Static {
            code: row.code,
            dest_class: row.dest_class,
            total_mass: row.total_mass,
            max_jump_mass: row.max_mass_per_jump,
            lifetime_hours: row.lifetime_hours,
            signature_strength: row.signature_strength,
        });
    }
    Ok(out)
}

/// `GET /api/systems/search?q=`, search the SDE solar systems by name. Prefix matches rank
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
        -- `!` on the columns an inner join makes certain: sqlx reads nullability out of the
        -- query plan, which changes shape on a table with no rows, so without these the
        -- build depends on whether the universe happens to be seeded.
        select s.id              as "id!",
               s.name            as "name!",
               s.security_status as "security!",
               r.name            as "region!",
               s.region_id       as "region_id!",
               s.constellation_id as "constellation_id!",
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
    let mut statics =
        statics_for(&state.db, &rows.iter().map(|r| r.id).collect::<Vec<_>>()).await?;
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
                statics: statics.remove(&row.id).unwrap_or_default(),
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

/// `GET /api/routing-graph`: the static half of the routing graph: k-space stargate
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
    // The legacy "essential" station services, each carrying its concrete stations so a
    // result can name the station. Security Offices (27) are a known quirk: only
    // CONCORD-owned lowsec stations actually run one, despite the operation listing the
    // service everywhere.
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

    // The NPC corporations that own stations, in the same shape as the services above, so
    // "the nearest Quafe Company station" is the same question as "the nearest repair shop"
    // and the client answers both with one relaxation over the graph it already has.
    let rows = sqlx::query!(
        r#"select st.owner_corporation_id as "corporation_id!", c.name as "corporation_name!",
                  st.id as "station_id", st.name as "station_name", st.solar_system_id
           from stations st
           join corporations c on c.id = st.owner_corporation_id
           where st.owner_corporation_id is not null
           order by c.name, st.name"#,
    )
    .fetch_all(&state.db)
    .await?;
    let corporations = group_stations(rows.into_iter().map(|row| {
        (
            row.corporation_id,
            row.corporation_name,
            serde_json::json!({
                "id": row.station_id,
                "name": row.station_name,
                "solar_system_id": row.solar_system_id,
            }),
        )
    }));

    Ok((
        [(axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
        Json(serde_json::json!({
            "adjacency": adjacency,
            "security": security,
            "jove": jove,
            "stations": stations,
            "services": services,
            "corporations": corporations,
        })),
    ))
}

/// Fold `(group id, group name, station)` rows, already ordered by group, into one entry
/// per group.
fn group_stations(
    rows: impl Iterator<Item = (i64, String, serde_json::Value)>,
) -> Vec<serde_json::Value> {
    let mut out: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<(i64, String, Vec<serde_json::Value>)> = None;
    for (id, name, station) in rows {
        match &mut current {
            Some((open, _, stations)) if *open == id => stations.push(station),
            _ => {
                if let Some((id, name, stations)) = current.take() {
                    out.push(serde_json::json!({ "id": id, "name": name, "stations": stations }));
                }
                current = Some((id, name, vec![station]));
            }
        }
    }
    if let Some((id, name, stations)) = current {
        out.push(serde_json::json!({ "id": id, "name": name, "stations": stations }));
    }
    out
}

/// `GET /api/systems/resolve?ids=a,b,c`, resolve solar system ids to display data for
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
    Ok(Json(systems_for(&state.db, &ids).await?))
}

/// Display data for a set of solar systems, in the shape every picker renders.
pub(super) async fn systems_for(
    db: &sqlx::PgPool,
    ids: &[i64],
) -> Result<Vec<SystemSearchResult>, ApiError> {
    let rows = sqlx::query!(
        r#"
        -- `!` on what the inner join makes certain: sqlx reads nullability out of the query
        -- plan, which changes shape on a table with no rows, so without these the build
        -- depends on whether the universe happens to be seeded yet.
        select s.id              as "id!",
               s.name            as "name!",
               s.security_status as "security!",
               r.name            as "region!",
               s.region_id       as "region_id!",
               s.constellation_id as "constellation_id!",
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
        ids,
    )
    .fetch_all(db)
    .await?;
    let mut statics = statics_for(db, &rows.iter().map(|r| r.id).collect::<Vec<_>>()).await?;
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
                statics: statics.remove(&row.id).unwrap_or_default(),
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
    Ok(results)
}

#[derive(Deserialize)]
pub struct ResolveQuery {
    pub ids: String,
}

/// `GET /api/threat/{solar_system_id}`: a wormhole system's threat analysis. 404 for
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

/// `GET /api/skyhooks`, every skyhook currently or shortly raidable. Public EVE data, so
/// no per-map gating; a session is still required, like the rest of the API.
pub async fn skyhooks(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Vec<crate::skyhooks::Skyhook>> {
    require_actor(&state.db, &jar).await?;
    Ok(Json(crate::skyhooks::list(&state.db).await?))
}

/// `GET /api/server-status`, what Tranquility is doing. Public: the header shows it
/// signed in or not, and it is the same figure ESI serves to anyone.
pub async fn server_status(
    State(state): State<AppState>,
) -> ApiResult<crate::server_status::ServerStatus> {
    Ok(Json(state.server.current()))
}
