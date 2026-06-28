//! Connections between placed systems: adding, removing, and marking the wormhole
//! life-cycle state of edges. All are Member+ (see [access.md](../../docs/database/access.md)).
//!
//! A connection carries its own `mass_status` / `time_status` / `size` so a hole can be
//! marked massed/EOL even before any signature is linked. Once a signature *is* linked, the
//! `map_*_sync` DB triggers (migration 0009) keep the connection and its signatures in
//! lock-step — worst-wins on link, verbatim on edit. See the
//! [sync spec](../../docs/database/mapping.md).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Used by the cross-target struct + command definitions.
use super::{ConnectionType, MassStatus, TimeStatus, WormholeSize};

#[cfg(feature = "ssr")]
use sqlx::PgPool;

#[cfg(feature = "ssr")]
use super::access::require_role;
#[cfg(feature = "ssr")]
use super::error::{MapError, Result};
#[cfg(feature = "ssr")]
use super::{Actor, Role};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConnection {
    pub id: i64,
    pub map_id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
    /// Wormhole life-cycle state. `None` = unknown; ignored for stargate edges. Synced with
    /// linked signatures by the DB triggers, so reads here already reflect the merged state.
    pub mass_status: Option<MassStatus>,
    pub time_status: Option<TimeStatus>,
    pub size: Option<WormholeSize>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddConnection {
    pub map_id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
}

#[cfg(feature = "ssr")]
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

/// Connect two placed systems. Endpoints are `map_solar_systems` ids, must be distinct and
/// both on this map. The same pair may be connected more than once (e.g. two separate
/// wormholes between the same systems), so duplicate edges are allowed. A new connection
/// starts with unknown life-cycle state.
#[cfg(feature = "ssr")]
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

    let connection = sqlx::query_as!(
        MapConnection,
        r#"insert into map_connections (map_id, from_system, to_system, type)
           values ($1, $2, $3, $4)
           returning id, map_id, from_system, to_system, type as "kind: ConnectionType",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     size as "size: WormholeSize",
                     created_at, updated_at"#,
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
        cmd.kind.as_str(),
    )
    .fetch_one(pool)
    .await?;
    Ok(connection)
}

/// A partial update of a connection's wormhole state. `None` leaves a field unchanged;
/// `Some(None)` clears it to unknown; `Some(Some(v))` sets it. Setting any field triggers
/// the DB sync, so linked signatures follow.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SetConnectionStatus {
    pub map_id: i64,
    pub connection_id: i64,
    #[serde(default)]
    pub mass_status: Option<Option<MassStatus>>,
    #[serde(default)]
    pub time_status: Option<Option<TimeStatus>>,
    #[serde(default)]
    pub size: Option<Option<WormholeSize>>,
}

/// Mark a connection's mass / EOL / size. Member+. Works whether or not a signature is
/// linked; when one is, the trigger propagates the change to it (and its sibling).
#[cfg(feature = "ssr")]
pub async fn set_connection_status(
    pool: &PgPool,
    actor: Actor,
    cmd: SetConnectionStatus,
) -> Result<MapConnection> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;

    let current = sqlx::query_as!(
        MapConnection,
        r#"select id, map_id, from_system, to_system, type as "kind: ConnectionType",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  size as "size: WormholeSize",
                  created_at, updated_at
           from map_connections where id = $1 and map_id = $2"#,
        cmd.connection_id,
        cmd.map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;

    let mass = cmd.mass_status.unwrap_or(current.mass_status);
    let time = cmd.time_status.unwrap_or(current.time_status);
    let size = cmd.size.unwrap_or(current.size);

    let connection = sqlx::query_as!(
        MapConnection,
        r#"update map_connections set mass_status = $1, time_status = $2, size = $3
           where id = $4 and map_id = $5
           returning id, map_id, from_system, to_system, type as "kind: ConnectionType",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     size as "size: WormholeSize",
                     created_at, updated_at"#,
        mass.map(|m| m.as_str()),
        time.map(|t| t.as_str()),
        size.map(|s| s.as_str()),
        cmd.connection_id,
        cmd.map_id,
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
#[cfg(feature = "ssr")]
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
