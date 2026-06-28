//! Graph-editing actions: placing/moving/removing systems and connecting them. All are
//! Member+ (see [access.md](../../docs/database/access.md)).

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::access::require_role;
use super::error::{MapError, Result};
use super::{Actor, ConnectionType, MapConnection, MapSolarSystem, Role};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddSystem {
    pub map_id: i64,
    pub solar_system_id: i64,
    pub x: f64,
    pub y: f64,
    pub alias: Option<String>,
}

/// Place a solar system on a map. The system must exist in the SDE and not already be on
/// the map. Adding a system does not touch its persisted details.
pub async fn add_system(pool: &PgPool, actor: Actor, cmd: AddSystem) -> Result<MapSolarSystem> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;

    let known = sqlx::query_scalar!(
        "select exists(select 1 from solar_systems where id = $1)",
        cmd.solar_system_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !known {
        return Err(MapError::Validation(format!(
            "unknown solar system {}",
            cmd.solar_system_id
        )));
    }

    let already = sqlx::query_scalar!(
        "select exists(select 1 from map_solar_systems where map_id = $1 and solar_system_id = $2)",
        cmd.map_id,
        cmd.solar_system_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if already {
        return Err(MapError::Conflict("system already on the map".into()));
    }

    let placed = sqlx::query_as!(
        MapSolarSystem,
        "insert into map_solar_systems (map_id, solar_system_id, position_x, position_y, alias)
         values ($1, $2, $3, $4, $5)
         returning id, map_id, solar_system_id, position_x, position_y, alias, created_at",
        cmd.map_id,
        cmd.solar_system_id,
        cmd.x,
        cmd.y,
        cmd.alias.as_deref(),
    )
    .fetch_one(pool)
    .await?;
    Ok(placed)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
}

/// Remove a system from a map. Cascades the system's signatures and any connections it
/// is an endpoint of; its persisted details survive.
pub async fn remove_system(pool: &PgPool, actor: Actor, cmd: RemoveSystem) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from map_solar_systems where id = $1 and map_id = $2",
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    if deleted == 0 {
        return Err(MapError::NotFound);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub x: f64,
    pub y: f64,
}

/// Move a placed system to a new position.
pub async fn move_system(pool: &PgPool, actor: Actor, cmd: MoveSystem) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "update map_solar_systems set position_x = $1, position_y = $2
         where id = $3 and map_id = $4",
        cmd.x,
        cmd.y,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(MapError::NotFound);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAlias {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub alias: Option<String>,
}

/// Set or clear a placement's ephemeral alias.
pub async fn set_alias(pool: &PgPool, actor: Actor, cmd: SetAlias) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "update map_solar_systems set alias = $1 where id = $2 and map_id = $3",
        cmd.alias.as_deref(),
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(MapError::NotFound);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddConnection {
    pub map_id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
}

impl AddConnection {
    pub fn validate(&self) -> Result<()> {
        if self.from_system == self.to_system {
            return Err(MapError::Validation(
                "cannot connect a system to itself".into(),
            ));
        }
        Ok(())
    }
}

/// Connect two placed systems. Endpoints are `map_solar_systems` ids, must be distinct,
/// both on this map, and not already connected (edges are unordered).
pub async fn add_connection(
    pool: &PgPool,
    actor: Actor,
    cmd: AddConnection,
) -> Result<MapConnection> {
    cmd.validate()?;
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;

    let on_map = sqlx::query_scalar!(
        "select count(*) from map_solar_systems where map_id = $1 and (id = $2 or id = $3)",
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);
    if on_map != 2 {
        return Err(MapError::Validation(
            "both endpoints must be systems on this map".into(),
        ));
    }

    let duplicate = sqlx::query_scalar!(
        "select exists(
             select 1 from map_connections
             where map_id = $1
               and ((from_system = $2 and to_system = $3) or (from_system = $3 and to_system = $2))
         )",
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if duplicate {
        return Err(MapError::Conflict(
            "those systems are already connected".into(),
        ));
    }

    let connection = sqlx::query_as!(
        MapConnection,
        r#"insert into map_connections (map_id, from_system, to_system, type)
           values ($1, $2, $3, $4)
           returning id, map_id, from_system, to_system, type as "kind: ConnectionType", created_at"#,
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
        cmd.kind.as_str(),
    )
    .fetch_one(pool)
    .await?;
    Ok(connection)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveConnection {
    pub map_id: i64,
    pub connection_id: i64,
}

/// Remove a connection. Linked signatures survive with their `connection_id` cleared.
pub async fn remove_connection(pool: &PgPool, actor: Actor, cmd: RemoveConnection) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from map_connections where id = $1 and map_id = $2",
        cmd.connection_id,
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    if deleted == 0 {
        return Err(MapError::NotFound);
    }
    Ok(())
}
