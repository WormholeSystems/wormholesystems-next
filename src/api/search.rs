//! Searching one map: systems, aliases, notes and recent kills, ranked together.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::extract::{ShareQuery, read_map_as};
use super::reference::{SearchQuery, statics_for, systems_for};
use super::{ApiError, ApiResult, SystemSearchResult};
use crate::auth::AppState;
use crate::maps::solar_system::sovereignty_of;

/// One hit from the map command palette. `map_solar_system_id` is set when the system is
/// already placed; otherwise the hit is an off-map system the palette can add.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapSearchHit {
    pub system: SystemSearchResult,
    pub map_solar_system_id: Option<i64>,
    pub alias: Option<String>,
    pub occupying_group: Option<String>,
    /// The matching slice of the system's notes, when the query hit the notes. Member+ only.
    pub note_excerpt: Option<String>,
    /// The organisation that made this system a hit, when the query named a threat group.
    #[ts(optional)]
    pub threat: Option<ThreatMatch>,
    /// Why this row matched: `name`, `alias`, `occupier`, `notes`, or `threat`.
    pub matched: String,
}

/// An organisation the threat analysis found operating in a system, as the palette shows it.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ThreatMatch {
    pub entity_id: i64,
    /// `alliance` or `corporation`.
    pub entity_type: String,
    pub name: String,
    /// Kills this organisation has in this system over the analysis window.
    pub kills: i32,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/maps/{id}/search", get(search_map))
}

/// `GET /api/maps/{id}/search?q=`: the map command palette. Matches placed systems by
/// name, alias, occupier and (for members) notes, then falls back to off-map systems the
/// palette can offer to add. Viewer+.
pub async fn search_map(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(query): Query<SearchQuery>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<MapSearchHit>> {
    let role = read_map_as(&state, &jar, map_id, &share).await?.role;
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    // Notes are member-gated everywhere else, so they must not leak through search.
    let with_notes = role >= crate::maps::Role::Member;
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");

    // The same columns the standalone system search selects, so both build an identical
    // SystemSearchResult.
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
    let mut statics = statics_for(
        &state.db,
        &placed.iter().map(|r| r.solar_system_id).collect::<Vec<_>>(),
    )
    .await?;
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
                    statics: statics.remove(&r.solar_system_id).unwrap_or_default(),
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
                threat: None,
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
    let mut off_statics = statics_for(
        &state.db,
        &off_map
            .iter()
            .map(|r| r.solar_system_id)
            .collect::<Vec<_>>(),
    )
    .await?;
    hits.extend(off_map.into_iter().map(|r| MapSearchHit {
        system: SystemSearchResult {
            statics: off_statics.remove(&r.solar_system_id).unwrap_or_default(),
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
        threat: None,
        matched: "name".into(),
    }));

    hits.extend(threat_hits(&state.db, map_id, q, &contains, &prefix).await?);
    Ok(Json(hits))
}

/// How many organisations a threat search considers, and how many systems it reports.
const THREAT_ENTITIES: i64 = 5;

const THREAT_SYSTEMS: usize = 12;

/// Systems where an organisation matching the query is a top killer, found from the
/// killmails rather than from occupiers anyone typed in.
///
/// Organisations are ranked exact match, then prefix, then anywhere in the name, and by
/// total kills within each tier, so a well-known alliance is not buried under the one-kill
/// corporations that happen to contain the same letters.
async fn threat_hits(
    db: &sqlx::PgPool,
    map_id: i64,
    needle: &str,
    contains: &str,
    prefix: &str,
) -> Result<Vec<MapSearchHit>, ApiError> {
    let rows = sqlx::query!(
        r#"with matched as (
               select entity_id, entity_type, name,
                      case when lower(name) = lower($1) then 0
                           when name ilike $3 then 1
                           else 2 end as rank,
                      sum(kills) as total
               from wormhole_system_threats
               where name ilike $2
               group by entity_id, entity_type, name
               order by rank, total desc
               limit $4
           )
           select t.solar_system_id, t.kills, m.entity_id, m.entity_type, m.name,
                  m.rank as "rank!", m.total as "total!"
           from wormhole_system_threats t
           join matched m on m.entity_id = t.entity_id
           order by m.rank, m.total desc, t.kills desc
           limit $5"#,
        needle,
        contains,
        prefix,
        THREAT_ENTITIES,
        THREAT_SYSTEMS as i64,
    )
    .fetch_all(db)
    .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<i64> = rows.iter().map(|r| r.solar_system_id).collect();
    let systems: std::collections::HashMap<i64, SystemSearchResult> = systems_for(db, &ids)
        .await?
        .into_iter()
        .map(|s| (s.id, s))
        .collect();
    // A threat system may already be on this map, in which case the row should jump to it
    // rather than offer to add it a second time.
    let placed: std::collections::HashMap<i64, i64> = sqlx::query!(
        r#"select id, solar_system_id as "solar_system_id!" from map_solar_systems
           where map_id = $1 and solar_system_id = any($2)"#,
        map_id,
        &ids,
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.solar_system_id, r.id))
    .collect();

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let system = systems.get(&r.solar_system_id)?.clone();
            Some(MapSearchHit {
                map_solar_system_id: placed.get(&r.solar_system_id).copied(),
                system,
                alias: None,
                occupying_group: None,
                note_excerpt: None,
                threat: Some(ThreatMatch {
                    entity_id: r.entity_id,
                    entity_type: r.entity_type,
                    name: r.name,
                    kills: r.kills,
                }),
                matched: "threat".into(),
            })
        })
        .collect())
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
