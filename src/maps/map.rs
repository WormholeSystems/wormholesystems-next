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
    let map = sqlx::query_as!(
        Map,
        "insert into maps (name, description) values ($1, $2)
         returning id, name, description, image_url, created_at",
        cmd.name.trim(),
        cmd.description.as_deref(),
    )
    .fetch_one(&mut *tx)
    .await?;
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
    tx.commit().await?;
    Ok(map)
}

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
}

impl UpdateMap {
    pub fn validate(&self) -> Result<()> {
        if let Some(name) = &self.name
            && name.trim().is_empty()
        {
            return Err(MapError::Validation("name must not be blank".into()));
        }
        Ok(())
    }
}

/// Rename / re-describe / re-icon a map. Owner only.
pub async fn update_map(pool: &PgPool, actor: Actor, cmd: UpdateMap) -> Result<Map> {
    cmd.validate()?;
    require_role(pool, cmd.map_id, actor.user_id, Role::Owner).await?;

    let mut tx = pool.begin().await?;
    let current = sqlx::query_as!(
        Map,
        "select id, name, description, image_url, created_at from maps where id = $1",
        cmd.map_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(MapError::NotFound)?;

    let name = match cmd.name {
        Some(name) => name.trim().to_string(),
        None => current.name,
    };
    let description = cmd.description.unwrap_or(current.description);
    let image_url = cmd.image_url.unwrap_or(current.image_url);

    let map = sqlx::query_as!(
        Map,
        "update maps set name = $1, description = $2, image_url = $3 where id = $4
         returning id, name, description, image_url, created_at",
        name,
        description,
        image_url,
        cmd.map_id,
    )
    .fetch_one(&mut *tx)
    .await?;
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
        r#"select m.id, m.name, m.description, m.image_url, m.created_at, ma.role as "role!: Role"
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
        let map = Map {
            id: r.id,
            name: r.name,
            description: r.description,
            image_url: r.image_url,
            created_at: r.created_at,
        };
        match out.iter_mut().find(|(m, _)| m.id == map.id) {
            Some((_, role)) => *role = (*role).max(r.role),
            None => out.push((map, r.role)),
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

    let map = sqlx::query_as!(
        Map,
        "select id, name, description, image_url, created_at from maps where id = $1",
        cmd.map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;

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
               ss.name as "name!", ss.security_status as "security_status!",
               ss.wormhole_class_id,
               r.name as "region!", ss.region_id,
               ss.constellation_id, c.name as "constellation!",
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
           join solar_systems ss on ss.id = mss.solar_system_id
           join regions r on r.id = ss.region_id
           join constellations c on c.id = ss.constellation_id
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
                statics: statics_by_system
                    .remove(&row.solar_system_id)
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
