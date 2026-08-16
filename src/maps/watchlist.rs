//! The per-map watchlist: systems whose jump distance the navigation panel tracks
//! (legacy `map_route_solarsystems`). Rows are map-scoped and shared; routing against
//! them happens client-side. Mutations are Member+; reads Viewer+.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::access::require_role;
use super::error::{MapError, Result};
use super::{Actor, Role};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct WatchlistEntry {
    pub id: i64,
    pub map_id: i64,
    pub solar_system_id: i64,
    pub is_pinned: bool,
}

/// Every watchlist entry on a map. Viewer+.
pub async fn list_watchlist(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
) -> Result<Vec<WatchlistEntry>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    let entries = sqlx::query_as!(
        WatchlistEntry,
        "select id, map_id, solar_system_id, is_pinned
         from map_watchlist where map_id = $1 order by id",
        map_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(entries)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AddWatchlistEntry {
    pub map_id: i64,
    pub solar_system_id: i64,
}

/// Watch a system on this map. Idempotent (unique per map+system). Member+.
pub async fn add_watchlist_entry(
    pool: &PgPool,
    actor: Actor,
    cmd: AddWatchlistEntry,
) -> Result<WatchlistEntry> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let entry = sqlx::query_as!(
        WatchlistEntry,
        "insert into map_watchlist (map_id, solar_system_id)
         values ($1, $2)
         on conflict (map_id, solar_system_id) do update set solar_system_id = excluded.solar_system_id
         returning id, map_id, solar_system_id, is_pinned",
        cmd.map_id,
        cmd.solar_system_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(entry)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetWatchlistPinned {
    pub map_id: i64,
    pub entry_id: i64,
    pub value: bool,
}

/// Pin/unpin a watchlist entry (pinned entries surface as route quick-picks). Member+.
pub async fn set_watchlist_pinned(
    pool: &PgPool,
    actor: Actor,
    cmd: SetWatchlistPinned,
) -> Result<WatchlistEntry> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    sqlx::query_as!(
        WatchlistEntry,
        "update map_watchlist set is_pinned = $1 where id = $2 and map_id = $3
         returning id, map_id, solar_system_id, is_pinned",
        cmd.value,
        cmd.entry_id,
        cmd.map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveWatchlistEntry {
    pub map_id: i64,
    pub entry_id: i64,
}

/// Stop watching. Member+.
pub async fn remove_watchlist_entry(
    pool: &PgPool,
    actor: Actor,
    cmd: RemoveWatchlistEntry,
) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from map_watchlist where id = $1 and map_id = $2",
        cmd.entry_id,
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
