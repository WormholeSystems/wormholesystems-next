//! The map entity: create, update, delete, list, and read the graph.
//!
//! Each mutating action takes a dedicated command struct (its future HTTP request body);
//! `actor` stays a separate argument, injected from the session — never the payload.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use sqlx::PgPool;

#[cfg(feature = "ssr")]
use super::access::{owns_character, require_role};
#[cfg(feature = "ssr")]
use super::error::{MapError, Result};
#[cfg(feature = "ssr")]
use super::{
    Actor, ConnectionType, MapConnection, MapSolarSystem, MapView, MassStatus, Role, SubjectType,
    TimeStatus, WormholeSize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMap {
    pub name: String,
    pub description: Option<String>,
}

#[cfg(feature = "ssr")]
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
#[cfg(feature = "ssr")]
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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct UpdateMap {
    pub map_id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub image_url: Option<Option<String>>,
}

#[cfg(feature = "ssr")]
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
#[cfg(feature = "ssr")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMap {
    pub map_id: i64,
}

/// Delete a map. Owner only. The database cascades placements, details, connections,
/// signatures, and access grants.
#[cfg(feature = "ssr")]
pub async fn delete_map(pool: &PgPool, actor: Actor, cmd: DeleteMap) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Owner).await?;
    sqlx::query!("delete from maps where id = $1", cmd.map_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Every map the user can access, paired with their effective role on it.
#[cfg(feature = "ssr")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMap {
    pub map_id: i64,
}

/// Read a map's graph — the map, its placed systems, and its connections. Viewer+.
/// Live pilot locations are not included (that's the member-gated tracking path).
#[cfg(feature = "ssr")]
pub async fn get_map(pool: &PgPool, actor: Actor, cmd: GetMap) -> Result<MapView> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Viewer).await?;

    let map = sqlx::query_as!(
        Map,
        "select id, name, description, image_url, created_at from maps where id = $1",
        cmd.map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;

    let systems = sqlx::query_as!(
        MapSolarSystem,
        "select id, map_id, solar_system_id, position_x, position_y, alias, created_at
         from map_solar_systems where map_id = $1 order by id",
        cmd.map_id,
    )
    .fetch_all(pool)
    .await?;

    let connections = sqlx::query_as!(
        MapConnection,
        r#"select id, map_id, from_system, to_system, type as "kind: ConnectionType",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  size as "size: WormholeSize",
                  created_at, updated_at
           from map_connections where map_id = $1 order by id"#,
        cmd.map_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(MapView {
        map,
        systems,
        connections,
    })
}
