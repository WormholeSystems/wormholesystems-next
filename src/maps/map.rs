//! The map entity: create, update, delete, list, and read the graph.
//!
//! Each mutating action takes a dedicated command struct (its future HTTP request body);
//! `actor` stays a separate argument, injected from the session — never the payload.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use sqlx::PgPool;

use super::access::{owns_character, require_role};
use super::error::{MapError, Result};
use std::collections::HashMap;

use super::solar_system::{MapSystemView, Sovereignty, Static};
use super::{
    Actor, ConnectionType, MapConnection, MapView, MassStatus, Role, SubjectType, TimeStatus,
    WormholeSize,
};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Map {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub naming: MapNaming,
    /// Whether pasting a wormhole signature puts its far side on the map as a ghost.
    /// Map-wide: a ghost is a node everyone on the chain sees.
    pub ghost_unlinked_wormholes: bool,
    /// How the chain is placed: `manual` (dragged into shape) or `tree` (drawn from the
    /// connections). Map-wide, so everyone on a chain reads the same picture.
    pub layout: String,
    /// Whether a viewer may pick their own placement instead of the map's.
    pub allow_layout_override: bool,
}

/// How a map names its chain. Map-wide rather than per-user, because an alias is written
/// on the map for everyone and a bookmark folder in three conventions is unreadable.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapNaming {
    /// `numeric` (`1`, `11`, `12`) or `alphabetical` (`A`, `AB`, with `H/L/N/P` reserved
    /// for k-space exits).
    pub alias_scheme: String,
    /// The alias that sits outside the chain, e.g. `HOME`.
    pub ignored_alias: String,
    pub bookmark_wormhole: String,
    pub bookmark_kspace: String,
    pub bookmark_return: String,
}

pub const ALIAS_SCHEMES: [&str; 2] = ["numeric", "alphabetical"];

impl MapNaming {
    pub fn validate(&self) -> Result<()> {
        if !ALIAS_SCHEMES.contains(&self.alias_scheme.as_str()) {
            return Err(MapError::Validation(format!(
                "unknown alias scheme {}",
                self.alias_scheme
            )));
        }
        // A blank format renders as an empty bookmark name, which the game silently
        // replaces with a generic one and the whole convention is lost.
        for (label, format) in [
            ("wormhole", &self.bookmark_wormhole),
            ("k-space", &self.bookmark_kspace),
            ("return", &self.bookmark_return),
        ] {
            if format.trim().is_empty() {
                return Err(MapError::Validation(format!(
                    "the {label} bookmark format must not be blank"
                )));
            }
        }
        Ok(())
    }
}

/// A row selected with the map columns, assembled into the nested shape the client sees.
macro_rules! map_from_row {
    ($row:expr) => {{
        let row = $row;
        Map {
            id: row.id,
            name: row.name,
            description: row.description,
            image_url: row.image_url,
            created_at: row.created_at,
            ghost_unlinked_wormholes: row.ghost_unlinked_wormholes,
            layout: row.layout,
            allow_layout_override: row.allow_layout_override,
            naming: MapNaming {
                alias_scheme: row.alias_scheme,
                ignored_alias: row.ignored_alias,
                bookmark_wormhole: row.bookmark_wormhole,
                bookmark_kspace: row.bookmark_kspace,
                bookmark_return: row.bookmark_return,
            },
        }
    }};
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CreateMap {
    pub name: String,
    pub description: Option<String>,
}

impl CreateMap {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(MapError::Validation("name must not be blank".into()));
        }
        Ok(())
    }
}

/// Create a map and grant ownership to the acting character. Any authenticated user may
/// create a map.
pub async fn create_map(pool: &PgPool, actor: Actor, cmd: CreateMap) -> Result<Map> {
    cmd.validate()?;
    if !owns_character(pool, actor.user_id, actor.character_id).await? {
        return Err(MapError::Forbidden);
    }

    let mut tx = pool.begin().await?;
    let map = map_from_row!(
        sqlx::query!(
            "insert into maps (name, description) values ($1, $2)
             returning id, name, description, image_url, created_at, alias_scheme, ignored_alias,
                 ghost_unlinked_wormholes, layout, allow_layout_override,
                 bookmark_wormhole, bookmark_kspace, bookmark_return",
            cmd.name.trim(),
            cmd.description.as_deref(),
        )
        .fetch_one(&mut *tx)
        .await?
    );
    sqlx::query!(
        "insert into map_access (map_id, subject_type, subject_id, role)
         values ($1, $2, $3, $4)",
        map.id,
        SubjectType::Character.as_str(),
        actor.character_id,
        Role::Owner.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    // Resolved by name rather than by id so an unseeded database quietly starts with an
    // empty watchlist instead of failing the whole creation on a foreign key.
    sqlx::query!(
        "insert into map_watchlist (map_id, solar_system_id, is_pinned)
         select $1, id, true from solar_systems where name = any($2)",
        map.id,
        &TRADE_HUBS.map(String::from)[..],
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(map)
}

/// Seeded onto every new map's watchlist, pinned, as legacy's `CreateMapAction` does. A
/// chain is worth mapping mostly for what it is close to, and that is nearly always one of
/// these; a map that starts empty makes you type them in before it can tell you anything.
const TRADE_HUBS: [&str; 5] = ["Jita", "Amarr", "Dodixie", "Rens", "Hek"];

/// A partial update of a map's fields. `None` leaves a field unchanged; `Some(None)`
/// explicitly clears a nullable field.
#[derive(Debug, Default, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateMap {
    pub map_id: i64,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub image_url: Option<Option<String>>,
    /// All-or-nothing: the naming block is edited as one form, so a partial payload here
    /// would only ever mean a half-saved form.
    #[serde(default)]
    #[ts(optional)]
    pub naming: Option<MapNaming>,
    #[serde(default)]
    #[ts(optional)]
    pub ghost_unlinked_wormholes: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub layout: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub allow_layout_override: Option<bool>,
}

impl UpdateMap {
    pub fn validate(&self) -> Result<()> {
        if let Some(name) = &self.name
            && name.trim().is_empty()
        {
            return Err(MapError::Validation("name must not be blank".into()));
        }
        if let Some(naming) = &self.naming {
            naming.validate()?;
        }
        if let Some(layout) = &self.layout
            && layout != "manual"
            && layout != "tree"
        {
            return Err(MapError::Validation(
                "placement must be manual or tree".into(),
            ));
        }
        Ok(())
    }
}

/// Rename / re-describe / re-icon a map. Owner only.
pub async fn update_map(pool: &PgPool, actor: Actor, cmd: UpdateMap) -> Result<Map> {
    cmd.validate()?;
    require_role(pool, cmd.map_id, actor.user_id, Role::Owner).await?;

    let mut tx = pool.begin().await?;
    let current = map_from_row!(
        sqlx::query!(
            "select id, name, description, image_url, created_at, alias_scheme, ignored_alias,
                 bookmark_wormhole, bookmark_kspace, bookmark_return, ghost_unlinked_wormholes,
                 layout, allow_layout_override
             from maps where id = $1",
            cmd.map_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(MapError::NotFound)?
    );

    let name = match cmd.name {
        Some(name) => name.trim().to_string(),
        None => current.name,
    };
    let description = cmd.description.unwrap_or(current.description);
    let image_url = cmd.image_url.unwrap_or(current.image_url);
    let naming = cmd.naming.unwrap_or(current.naming);
    let ghost_unlinked_wormholes = cmd
        .ghost_unlinked_wormholes
        .unwrap_or(current.ghost_unlinked_wormholes);
    let layout = cmd.layout.unwrap_or(current.layout);
    let allow_layout_override = cmd
        .allow_layout_override
        .unwrap_or(current.allow_layout_override);

    let map = map_from_row!(
        sqlx::query!(
            "update maps set name = $1, description = $2, image_url = $3,
                    alias_scheme = $5, ignored_alias = $6, bookmark_wormhole = $7,
                    bookmark_kspace = $8, bookmark_return = $9, ghost_unlinked_wormholes = $10,
                    layout = $11, allow_layout_override = $12
             where id = $4
             returning id, name, description, image_url, created_at, alias_scheme, ignored_alias,
                 bookmark_wormhole, bookmark_kspace, bookmark_return, ghost_unlinked_wormholes,
                 layout, allow_layout_override",
            name,
            description,
            image_url,
            cmd.map_id,
            naming.alias_scheme,
            naming.ignored_alias,
            naming.bookmark_wormhole,
            naming.bookmark_kspace,
            naming.bookmark_return,
            ghost_unlinked_wormholes,
            layout,
            allow_layout_override,
        )
        .fetch_one(&mut *tx)
        .await?
    );
    tx.commit().await?;
    Ok(map)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct DeleteMap {
    pub map_id: i64,
}

/// Delete a map. Owner only. The database cascades placements, details, connections,
/// signatures, and access grants.
pub async fn delete_map(pool: &PgPool, actor: Actor, cmd: DeleteMap) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Owner).await?;
    sqlx::query!("delete from maps where id = $1", cmd.map_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Every map the user can access, paired with their effective role on it.
pub async fn list_maps(pool: &PgPool, user_id: i64) -> Result<Vec<(Map, Role)>> {
    let rows = sqlx::query!(
        r#"select m.id, m.name, m.description, m.image_url, m.created_at, m.alias_scheme,
                  m.ignored_alias, m.bookmark_wormhole, m.bookmark_kspace, m.bookmark_return,
                  m.ghost_unlinked_wormholes, m.layout, m.allow_layout_override,
                  ma.role as "role!: Role"
           from maps m
           join map_access ma on ma.map_id = m.id
           where ma.subject_id in (
                 select id from characters where user_id = $1
                 union all
                 select corporation_id from characters where user_id = $1
                 union all
                 select alliance_id from characters where user_id = $1 and alliance_id is not null
           )
           order by m.created_at"#,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    // A map can match more than once (e.g. via a character and its corp); keep the
    // highest role per map.
    let mut out: Vec<(Map, Role)> = Vec::new();
    for r in rows {
        let role = r.role;
        let map = map_from_row!(r);
        match out.iter_mut().find(|(m, _)| m.id == map.id) {
            Some((_, existing)) => *existing = (*existing).max(role),
            None => out.push((map, role)),
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct GetMap {
    pub map_id: i64,
}

/// Read a map's graph — the map, its placed systems, and its connections. Viewer+.
/// Live pilot locations are not included (that's the member-gated tracking path).
pub async fn get_map(pool: &PgPool, actor: Actor, cmd: GetMap) -> Result<MapView> {
    let role = require_role(pool, cmd.map_id, actor.user_id, Role::Viewer).await?;

    let map = map_from_row!(
        sqlx::query!(
            "select id, name, description, image_url, created_at, alias_scheme, ignored_alias,
                 bookmark_wormhole, bookmark_kspace, bookmark_return, ghost_unlinked_wormholes,
                 layout, allow_layout_override
             from maps where id = $1",
            cmd.map_id,
        )
        .fetch_optional(pool)
        .await?
        .ok_or(MapError::NotFound)?
    );

    // Statics are 1-to-many, so fetch them in one query for the whole map and group by
    // system rather than joining (which would multiply the system rows).
    let mut statics_by_system: HashMap<i64, Vec<Static>> = HashMap::new();
    let static_rows = sqlx::query!(
        "select wss.solar_system_id, wt.code, wt.dest_class,
                wt.total_mass, wt.max_mass_per_jump, wt.lifetime_hours, wt.signature_strength
         from map_solar_systems mss
         join wormhole_system_statics wss on wss.solar_system_id = mss.solar_system_id
         join wormhole_types wt on wt.code = wss.wormhole_code
         where mss.map_id = $1",
        cmd.map_id,
    )
    .fetch_all(pool)
    .await?;
    for row in static_rows {
        statics_by_system
            .entry(row.solar_system_id)
            .or_default()
            .push(Static {
                code: row.code,
                dest_class: row.dest_class,
                total_mass: row.total_mass,
                max_jump_mass: row.max_mass_per_jump,
                lifetime_hours: row.lifetime_hours,
                signature_strength: row.signature_strength,
            });
    }

    let rows = sqlx::query!(
        r#"select
               mss.id, mss.map_id, mss.solar_system_id, mss.position_x, mss.position_y,
               mss.alias, mss.is_home, mss.is_rally, mss.is_pinned,
               coalesce(d.status, 'unknown') as "status!: super::SystemStatus",
               d.occupying_group,
               ss.name as "name?", ss.security_status as "security_status?",
               ss.wormhole_class_id,
               r.name as "region?", ss.region_id as "region_id?",
               ss.constellation_id as "constellation_id?", c.name as "constellation?",
               ws.effect_name,
               coalesce(ws.is_shattered, false) as "is_shattered!",
               ws.threat_level as "threat_level?: super::ThreatLevel",
               -- Sovereignty holder, alliance preferred over corp over faction.
               case
                   when sov.alliance_id is not null then 'alliance'
                   when sov.corporation_id is not null then 'corporation'
                   when sov.faction_id is not null then 'faction'
               end as "sov_kind?",
               coalesce(sov.alliance_id, sov.corporation_id, sov.faction_id) as "sov_id?",
               coalesce(al.name, co.name, f.name) as "sov_name?",
               coalesce(al.ticker, co.ticker) as "sov_ticker?"
           from map_solar_systems mss
           -- Left joins throughout: a ghost placement has no system to join to.
           left join solar_systems ss on ss.id = mss.solar_system_id
           left join regions r on r.id = ss.region_id
           left join constellations c on c.id = ss.constellation_id
           left join map_solar_system_details d
               on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
           left join wormhole_systems ws on ws.solar_system_id = ss.id
           left join system_sovereignty sov on sov.solar_system_id = ss.id
           left join alliances al on al.id = sov.alliance_id
           left join corporations co on co.id = sov.corporation_id
           left join factions f on f.id = sov.faction_id
           where mss.map_id = $1
           order by mss.id"#,
        cmd.map_id,
    )
    .fetch_all(pool)
    .await?;

    let systems = rows
        .into_iter()
        .map(|row| {
            // kind/id/name are present together (or all absent). Ticker exists for
            // alliances/corps; factions have none.
            let sovereignty = match (row.sov_kind.as_deref(), row.sov_id, row.sov_name) {
                (Some("alliance"), Some(id), Some(name)) => Some(Sovereignty::Alliance {
                    id,
                    name,
                    ticker: row.sov_ticker.unwrap_or_default(),
                }),
                (Some("corporation"), Some(id), Some(name)) => Some(Sovereignty::Corporation {
                    id,
                    name,
                    ticker: row.sov_ticker.unwrap_or_default(),
                }),
                (Some("faction"), Some(id), Some(name)) => Some(Sovereignty::Faction { id, name }),
                _ => None,
            };
            MapSystemView {
                id: row.id,
                map_id: row.map_id,
                solar_system_id: row.solar_system_id,
                position_x: row.position_x,
                position_y: row.position_y,
                alias: row.alias,
                is_home: row.is_home,
                is_rally: row.is_rally,
                is_pinned: row.is_pinned,
                status: row.status,
                occupying_group: row.occupying_group,
                name: row.name,
                security_status: row.security_status,
                wormhole_class_id: row.wormhole_class_id,
                region: row.region,
                region_id: row.region_id,
                constellation_id: row.constellation_id,
                constellation: row.constellation,
                effect_name: row.effect_name,
                is_shattered: row.is_shattered,
                threat_level: row.threat_level,
                statics: row
                    .solar_system_id
                    .and_then(|id| statics_by_system.remove(&id))
                    .unwrap_or_default(),
                sovereignty,
            }
        })
        .collect();

    let connections = sqlx::query_as!(
        MapConnection,
        r#"select id, map_id, from_system, to_system, type as "kind: ConnectionType",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  size as "size: WormholeSize",
                  (select count(*) from map_connection_jumps j
                   where j.connection_id = map_connections.id) as "jumps_count!",
                  coalesce((select sum(j.mass) from map_connection_jumps j
                            where j.connection_id = map_connections.id), 0)::bigint as "jumps_mass_sum!",
                  preserve_mass, time_status_updated_at, created_at, updated_at
           from map_connections where map_id = $1 order by id"#,
        cmd.map_id,
    )
    .fetch_all(pool)
    .await?;

    let character_has_access = sqlx::query_scalar!(
        r#"select exists(
               select 1 from map_access a
               join characters c on c.id = $2
               where a.map_id = $1
                 and a.subject_id in (c.id, c.corporation_id, c.alliance_id)
           ) as "has!""#,
        cmd.map_id,
        actor.character_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(MapView {
        map,
        role,
        character_has_access,
        systems,
        connections,
    })
}
