//! Placed solar systems: placing, moving, removing, and aliasing systems on a map. All
//! are Member+ (see [access.md](../../docs/database/access.md)).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use sqlx::PgPool;

#[cfg(feature = "ssr")]
use super::access::require_role;
#[cfg(feature = "ssr")]
use super::error::{MapError, Result};
#[cfg(feature = "ssr")]
use super::{Actor, Role};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSolarSystem {
    pub id: i64,
    pub map_id: i64,
    pub solar_system_id: i64,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub created_at: DateTime<Utc>,
}

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
#[cfg(feature = "ssr")]
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
#[cfg(feature = "ssr")]
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
pub struct RemoveSystems {
    pub map_id: i64,
    pub map_solar_system_ids: Vec<i64>,
}

/// Remove several placed systems at once (multi-select delete). Same cascade as
/// [`remove_system`]. Returns the number actually removed; an empty id list is a no-op.
#[cfg(feature = "ssr")]
pub async fn remove_systems(pool: &PgPool, actor: Actor, cmd: RemoveSystems) -> Result<u64> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from map_solar_systems where map_id = $1 and id = any($2)",
        cmd.map_id,
        &cmd.map_solar_system_ids,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(deleted)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearMap {
    pub map_id: i64,
}

/// Remove every placed system on a map except the home system and any pinned systems.
/// Connections to removed systems cascade. Returns the number removed.
#[cfg(feature = "ssr")]
pub async fn clear_map(pool: &PgPool, actor: Actor, cmd: ClearMap) -> Result<u64> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from map_solar_systems where map_id = $1 and not is_home and not is_pinned",
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(deleted)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub x: f64,
    pub y: f64,
}

/// Move a placed system to a new position.
#[cfg(feature = "ssr")]
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
#[cfg(feature = "ssr")]
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
pub struct SetStatus {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub status: super::SystemStatus,
}

/// Set a placed system's intel status. Upserts the persisted details row, keyed by the
/// placement's `(map_id, solar_system_id)`.
#[cfg(feature = "ssr")]
pub async fn set_status(pool: &PgPool, actor: Actor, cmd: SetStatus) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "insert into map_solar_system_details (map_id, solar_system_id, status)
         select map_id, solar_system_id, $3 from map_solar_systems where id = $2 and map_id = $1
         on conflict (map_id, solar_system_id)
             do update set status = excluded.status, updated_at = now()",
        cmd.map_id,
        cmd.map_solar_system_id,
        cmd.status.as_str(),
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
pub struct SetOccupier {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub occupier: Option<String>,
}

/// Set or clear who occupies a placed system (free text, like the alias). Upserts the
/// persisted details row.
#[cfg(feature = "ssr")]
pub async fn set_occupier(pool: &PgPool, actor: Actor, cmd: SetOccupier) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "insert into map_solar_system_details (map_id, solar_system_id, occupying_group)
         select map_id, solar_system_id, $3 from map_solar_systems where id = $2 and map_id = $1
         on conflict (map_id, solar_system_id)
             do update set occupying_group = excluded.occupying_group, updated_at = now()",
        cmd.map_id,
        cmd.map_solar_system_id,
        cmd.occupier.as_deref(),
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
pub struct SetHome {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub value: bool,
}

/// Mark a placement as the map's home system (or clear it). A map has at most one home (a
/// partial unique index enforces it), so setting a new home first clears the previous one.
#[cfg(feature = "ssr")]
pub async fn set_home(pool: &PgPool, actor: Actor, cmd: SetHome) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let mut tx = pool.begin().await?;
    if cmd.value {
        sqlx::query!(
            "update map_solar_systems set is_home = false where map_id = $1 and is_home",
            cmd.map_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    let updated = sqlx::query!(
        "update map_solar_systems set is_home = $1 where id = $2 and map_id = $3",
        cmd.value,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        // Drop the transaction without committing — the home-clear above rolls back.
        return Err(MapError::NotFound);
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetPinned {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub value: bool,
}

/// Pin or unpin a placement. Pinned systems are drag-locked client-side and survive
/// "clear map". Any number of systems may be pinned.
#[cfg(feature = "ssr")]
pub async fn set_pinned(pool: &PgPool, actor: Actor, cmd: SetPinned) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "update map_solar_systems set is_pinned = $1 where id = $2 and map_id = $3",
        cmd.value,
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
