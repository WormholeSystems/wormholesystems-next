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
use super::Role;
use super::access::require_role;
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

/// How long a connection must have been marked critical before the map offers to sweep it.
/// Legacy's rule: a critical hole is minutes from collapsing, so an hour without an update
/// means nobody is flying it any more.
pub const STALE_AFTER_MINUTES: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CleanStaleConnections {
    pub map_id: i64,
}

/// One stale edge, for the status bar's cleanup popover.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct StaleConnection {
    pub connection_id: i64,
    pub from_name: String,
    pub to_name: String,
    pub critical_since: DateTime<Utc>,
}

/// Every connection that has been critical for longer than [`STALE_AFTER_MINUTES`]. Viewer+.
pub async fn list_stale_connections(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
) -> Result<Vec<StaleConnection>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    let rows = sqlx::query_as!(
        StaleConnection,
        r#"select c.id as "connection_id!",
                  fs.name as "from_name!", ts.name as "to_name!",
                  c.time_status_updated_at as "critical_since!"
           from map_connections c
           join map_solar_systems fm on fm.id = c.from_system
           join solar_systems fs on fs.id = fm.solar_system_id
           join map_solar_systems tm on tm.id = c.to_system
           join solar_systems ts on ts.id = tm.solar_system_id
           where c.map_id = $1
             and c.time_status = 'critical'
             and c.time_status_updated_at < now() - make_interval(mins => $2)
           order by c.time_status_updated_at"#,
        map_id,
        STALE_AFTER_MINUTES as i32,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Sweep every stale connection, plus any placement they leave behind. Member+. Recorded
/// as one entry, so a sweep that went too far is a single undo.
pub async fn clean_stale_connections(
    pool: &PgPool,
    actor: Actor,
    cmd: CleanStaleConnections,
) -> Result<u64> {
    match execute(pool, actor, MapCommand::CleanStaleConnections(cmd)).await? {
        CommandOutput::Count(n) => Ok(n),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_clean_stale(
    tx: &mut Tx<'_>,
    cmd: CleanStaleConnections,
) -> Result<Effect> {
    let stale = sqlx::query!(
        r#"select id as "id!", from_system as "from_system!", to_system as "to_system!"
           from map_connections
           where map_id = $1
             and time_status = 'critical'
             and time_status_updated_at < now() - make_interval(mins => $2)"#,
        cmd.map_id,
        STALE_AFTER_MINUTES as i32,
    )
    .fetch_all(&mut **tx)
    .await?;
    if stale.is_empty() {
        return Err(MapError::Conflict("nothing to clean".into()));
    }

    // Snapshot the edges before deleting them, then the placements they orphan. Order
    // matters: a placement only looks orphaned once its stale edges are gone.
    let mut snapshot = super::solar_system::RestoreSystems {
        map_id: cmd.map_id,
        systems: Vec::new(),
        connections: Vec::new(),
        signatures: Vec::new(),
    };
    for row in &stale {
        if let Some(one) = super::solar_system::capture_connection(tx, cmd.map_id, row.id).await? {
            snapshot.connections.extend(one.connections);
        }
    }
    let stale_ids: Vec<i64> = stale.iter().map(|r| r.id).collect();
    sqlx::query!(
        "delete from map_connections where map_id = $1 and id = any($2)",
        cmd.map_id,
        &stale_ids,
    )
    .execute(&mut **tx)
    .await?;

    let mut endpoints: Vec<i64> = stale
        .iter()
        .flat_map(|r| [r.from_system, r.to_system])
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();

    // Same orphan rule as the signature cascade: unpinned, unmarked, and now edgeless.
    let orphans = sqlx::query_scalar!(
        r#"select id as "id!" from map_solar_systems mss
           where mss.map_id = $1 and mss.id = any($2)
             and not mss.is_pinned and not mss.is_home and not mss.is_rally
             and not exists (
                 select 1 from map_connections c
                 where c.map_id = $1 and (c.from_system = mss.id or c.to_system = mss.id)
             )"#,
        cmd.map_id,
        &endpoints,
    )
    .fetch_all(&mut **tx)
    .await?;

    if !orphans.is_empty() {
        let captured = super::solar_system::capture_systems(tx, cmd.map_id, &orphans).await?;
        snapshot.systems = captured.systems;
        snapshot.signatures = captured.signatures;
        sqlx::query!(
            "delete from map_solar_systems where map_id = $1 and id = any($2)",
            cmd.map_id,
            &orphans,
        )
        .execute(&mut **tx)
        .await?;
    }

    let removed = stale.len() as u64;
    let label = match (removed, orphans.len()) {
        (1, 0) => "cleaned a stale connection".to_string(),
        (n, 0) => format!("cleaned {n} stale connections"),
        (1, m) => format!("cleaned a stale connection and {m} system(s)"),
        (n, m) => format!("cleaned {n} stale connections and {m} system(s)"),
    };
    Ok(
        Effect::new("connections.cleaned", label, CommandOutput::Count(removed))
            .entries(removed as i64 + orphans.len() as i64)
            .undo_with(MapCommand::RestoreSystems(snapshot)),
    )
}
