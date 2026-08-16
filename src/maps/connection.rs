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

use sqlx::PgPool;

use super::Actor;
use super::command::{CommandOutput, Effect, MapCommand, Tx, execute};
use super::error::{MapError, Result};
use super::solar_system::unexpected;

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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
    /// Exclude this hole from mass bookkeeping (legacy flag; stored, not yet consumed).
    pub preserve_mass: bool,
    /// When `time_status` last changed (DB trigger), for "EOL since" displays.
    pub time_status_updated_at: Option<DateTime<Utc>>,
    /// Full jump-log aggregates (the log itself is fetched separately, capped at 10).
    pub jumps_count: i64,
    pub jumps_mass_sum: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AddConnection {
    pub map_id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
    /// Initial max ship size, when the client can infer it (e.g. a C13 endpoint is
    /// frigate-sized). `None` = unknown.
    #[serde(default)]
    #[ts(optional)]
    pub size: Option<WormholeSize>,
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

/// Connect two placed systems. Endpoints are `map_solar_systems` ids, must be distinct and
/// both on this map. The same pair may be connected more than once (e.g. two separate
/// wormholes between the same systems), so duplicate edges are allowed. A new connection
/// starts with unknown life-cycle state.
pub async fn add_connection(
    pool: &PgPool,
    actor: Actor,
    cmd: AddConnection,
) -> Result<MapConnection> {
    match execute(pool, actor, MapCommand::AddConnection(cmd)).await? {
        CommandOutput::Connection(c) => Ok(*c),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_add_connection(tx: &mut Tx<'_>, cmd: AddConnection) -> Result<Effect> {
    cmd.validate()?;

    let on_map = sqlx::query_scalar!(
        "select count(*) from map_solar_systems where map_id = $1 and (id = $2 or id = $3)",
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(0);
    if on_map != 2 {
        return Err(MapError::Validation(
            "both endpoints must be systems on this map".into(),
        ));
    }

    let connection = sqlx::query_as!(
        MapConnection,
        r#"insert into map_connections (map_id, from_system, to_system, type, size)
           values ($1, $2, $3, $4, $5)
           returning id, map_id, from_system, to_system, type as "kind: ConnectionType",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     size as "size: WormholeSize",
                     (select count(*) from map_connection_jumps j
                      where j.connection_id = map_connections.id) as "jumps_count!",
                     coalesce((select sum(j.mass) from map_connection_jumps j
                               where j.connection_id = map_connections.id), 0)::bigint as "jumps_mass_sum!",
                     preserve_mass, time_status_updated_at, created_at, updated_at"#,
        cmd.map_id,
        cmd.from_system,
        cmd.to_system,
        cmd.kind.as_str(),
        cmd.size.map(|s| s.as_str()),
    )
    .fetch_one(&mut **tx)
    .await?;

    // A hole often gets jumped moments before it's mapped: adopt those observations.
    if connection.kind == ConnectionType::Wormhole {
        let endpoints = sqlx::query!(
            "select f.solar_system_id as from_sys, t.solar_system_id as to_sys
             from map_solar_systems f, map_solar_systems t
             where f.id = $1 and t.id = $2",
            connection.from_system,
            connection.to_system,
        )
        .fetch_one(&mut **tx)
        .await?;
        super::jumps::claim_pending_tx(
            tx,
            connection.map_id,
            connection.id,
            endpoints.from_sys,
            endpoints.to_sys,
        )
        .await?;
    }
    let inverse = MapCommand::RemoveConnection(RemoveConnection {
        map_id: connection.map_id,
        connection_id: connection.id,
    });
    Ok(Effect::new(
        "connections.added",
        "mapped a connection",
        CommandOutput::Connection(Box::new(connection)),
    )
    .undo_with(inverse))
}

/// A partial update of a connection's wormhole state. `None` leaves a field unchanged;
/// `Some(None)` clears it to unknown; `Some(Some(v))` sets it. Setting any field triggers
/// the DB sync, so linked signatures follow.
#[derive(Debug, Default, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetConnectionStatus {
    pub map_id: i64,
    pub connection_id: i64,
    /// `None` leaves the edge kind unchanged; `Some(k)` switches wormhole/stargate.
    #[serde(default)]
    #[ts(optional)]
    pub kind: Option<ConnectionType>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub mass_status: Option<Option<MassStatus>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub time_status: Option<Option<TimeStatus>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub size: Option<Option<WormholeSize>>,
    /// `None` leaves the preserve-mass flag unchanged.
    #[serde(default)]
    #[ts(optional)]
    pub preserve_mass: Option<bool>,
}

/// Mark a connection's mass / EOL / size. Member+. Works whether or not a signature is
/// linked; when one is, the trigger propagates the change to it (and its sibling).
pub async fn set_connection_status(
    pool: &PgPool,
    actor: Actor,
    cmd: SetConnectionStatus,
) -> Result<MapConnection> {
    match execute(pool, actor, MapCommand::SetConnectionStatus(cmd)).await? {
        CommandOutput::Connection(c) => Ok(*c),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_set_connection_status(
    tx: &mut Tx<'_>,
    cmd: SetConnectionStatus,
) -> Result<Effect> {
    let current = sqlx::query_as!(
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
           from map_connections where id = $1 and map_id = $2"#,
        cmd.connection_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;

    let kind = cmd.kind.unwrap_or(current.kind);
    let mass = cmd.mass_status.unwrap_or(current.mass_status);
    let time = cmd.time_status.unwrap_or(current.time_status);
    let size = cmd.size.unwrap_or(current.size);
    let preserve = cmd.preserve_mass.unwrap_or(current.preserve_mass);

    let connection = sqlx::query_as!(
        MapConnection,
        r#"update map_connections
           set type = $1, mass_status = $2, time_status = $3, size = $4, preserve_mass = $5
           where id = $6 and map_id = $7
           returning id, map_id, from_system, to_system, type as "kind: ConnectionType",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     size as "size: WormholeSize",
                     (select count(*) from map_connection_jumps j
                      where j.connection_id = map_connections.id) as "jumps_count!",
                     coalesce((select sum(j.mass) from map_connection_jumps j
                               where j.connection_id = map_connections.id), 0)::bigint as "jumps_mass_sum!",
                     preserve_mass, time_status_updated_at, created_at, updated_at"#,
        kind.as_str(),
        mass.map(|m| m.as_str()),
        time.map(|t| t.as_str()),
        size.map(|s| s.as_str()),
        preserve,
        cmd.connection_id,
        cmd.map_id,
    )
    .fetch_one(&mut **tx)
    .await?;
    // Undo restores every field, since the sync triggers may have moved more than one.
    let inverse = MapCommand::SetConnectionStatus(SetConnectionStatus {
        map_id: cmd.map_id,
        connection_id: cmd.connection_id,
        kind: Some(current.kind),
        mass_status: Some(current.mass_status),
        time_status: Some(current.time_status),
        size: Some(current.size),
        preserve_mass: Some(current.preserve_mass),
    });
    Ok(Effect::new(
        "connections.updated",
        "updated a connection",
        CommandOutput::Connection(Box::new(connection)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveConnection {
    pub map_id: i64,
    pub connection_id: i64,
}

/// Remove a connection. Linked signatures survive with their `connection_id` cleared.
pub async fn remove_connection(pool: &PgPool, actor: Actor, cmd: RemoveConnection) -> Result<()> {
    execute(pool, actor, MapCommand::RemoveConnection(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_remove_connection(
    tx: &mut Tx<'_>,
    cmd: RemoveConnection,
) -> Result<Effect> {
    let restore = super::solar_system::capture_connection(tx, cmd.map_id, cmd.connection_id)
        .await?
        .ok_or(MapError::NotFound)?;
    sqlx::query!(
        "delete from map_connections where id = $1 and map_id = $2",
        cmd.connection_id,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;
    Ok(Effect::new(
        "connections.removed",
        "removed a connection",
        CommandOutput::None,
    )
    .undo_with(MapCommand::RestoreSystems(restore)))
}
