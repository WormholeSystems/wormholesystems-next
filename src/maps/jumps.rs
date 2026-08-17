//! Jump tracking for wormhole connections: automatic transit capture from character
//! location polling plus manual entries, feeding the connection's mass-remaining
//! estimate (`jumps_count` / `jumps_mass_sum` on [`MapConnection`](super::MapConnection)).
//!
//! Automatic capture ([`record_transit`]) runs on every observed system change of a
//! tracked character: stargate hops are ignored, and on each map where the character's
//! user opted into tracking (Member+), the transit lands on the matching wormhole
//! connection — or as a **pending** row (`connection_id` null) when the hole isn't
//! mapped yet. Pending rows are claimed by a connection created within 120 seconds
//! ([`claim_pending`]) and pruned after 10 minutes otherwise. Masses are hull masses
//! from the seeded `types` table; in game the effective jump mass varies by ±10%.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::access::{effective_role, require_role};
use super::command::{CommandOutput, Effect, MapCommand, Tx, execute};
use super::error::{MapError, Result};
use super::solar_system::unexpected;
use super::{Actor, MapEvent, MapHub, Role};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ConnectionJump {
    pub id: i64,
    pub map_id: i64,
    pub connection_id: Option<i64>,
    pub character_id: Option<i64>,
    pub character_name: Option<String>,
    pub ship_type_id: Option<i64>,
    pub ship_type_name: Option<String>,
    /// Hull mass in kg.
    pub mass: i64,
    pub is_manual: bool,
    pub from_solar_system_id: i64,
    pub to_solar_system_id: i64,
    pub created_at: DateTime<Utc>,
}

/// Which way a manual jump went, relative to the connection's `from_system` endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum JumpDirection {
    Outbound,
    Inbound,
}

/// A connection's endpoint solar systems, in `from_system` → `to_system` order.
async fn connection_endpoints(
    tx: &mut Tx<'_>,
    map_id: i64,
    connection_id: i64,
) -> Result<(i64, i64)> {
    let row = sqlx::query!(
        "select f.solar_system_id as from_sys, t.solar_system_id as to_sys
         from map_connections c
         join map_solar_systems f on f.id = c.from_system
         join map_solar_systems t on t.id = c.to_system
         where c.id = $1 and c.map_id = $2",
        connection_id,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;
    Ok((row.from_sys, row.to_sys))
}

/// A ship type's hull mass (kg, rounded), or `NotFound`-flavored validation.
async fn ship_mass_tx(tx: &mut Tx<'_>, ship_type_id: i64) -> Result<i64> {
    ship_mass(&mut **tx, ship_type_id).await
}

async fn ship_mass<'e, E>(executor: E, ship_type_id: i64) -> Result<i64>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let mass = sqlx::query_scalar!("select mass from types where id = $1", ship_type_id)
        .fetch_optional(executor)
        .await?
        .ok_or_else(|| MapError::Validation("unknown ship type".into()))?;
    Ok(mass.unwrap_or(0.0).round() as i64)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AddConnectionJump {
    pub map_id: i64,
    pub connection_id: i64,
    pub direction: JumpDirection,
    #[serde(default)]
    #[ts(optional)]
    pub ship_type_id: Option<i64>,
    /// Explicit mass in kg; wins over the ship type's hull mass.
    #[serde(default)]
    #[ts(optional)]
    pub mass: Option<i64>,
}

/// Log a jump manually. Member+. Needs a mass or a ship type (whose hull mass is used).
pub async fn add_jump(
    pool: &PgPool,
    actor: Actor,
    cmd: AddConnectionJump,
) -> Result<ConnectionJump> {
    match execute(pool, actor, MapCommand::AddConnectionJump(cmd)).await? {
        CommandOutput::Jump(j) => Ok(*j),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_add_jump(tx: &mut Tx<'_>, cmd: AddConnectionJump) -> Result<Effect> {
    if cmd.mass.is_none() && cmd.ship_type_id.is_none() {
        return Err(MapError::Validation(
            "enter a mass or pick a ship type".into(),
        ));
    }
    if cmd.mass.is_some_and(|m| m < 0) {
        return Err(MapError::Validation("mass must not be negative".into()));
    }
    let (from_sys, to_sys) = connection_endpoints(tx, cmd.map_id, cmd.connection_id).await?;
    let (from, to) = match cmd.direction {
        JumpDirection::Outbound => (from_sys, to_sys),
        JumpDirection::Inbound => (to_sys, from_sys),
    };
    let mass = match cmd.mass {
        Some(m) => m,
        None => ship_mass_tx(tx, cmd.ship_type_id.expect("checked above")).await?,
    };

    let id = sqlx::query_scalar!(
        "insert into map_connection_jumps
             (map_id, connection_id, from_solar_system_id, to_solar_system_id,
              ship_type_id, mass, is_manual)
         values ($1, $2, $3, $4, $5, $6, true)
         returning id",
        cmd.map_id,
        cmd.connection_id,
        from,
        to,
        cmd.ship_type_id,
        mass,
    )
    .fetch_one(&mut **tx)
    .await?;
    let jump = fetch_jump_tx(tx, cmd.map_id, id).await?;
    let inverse = MapCommand::RemoveConnectionJump(RemoveConnectionJump {
        map_id: cmd.map_id,
        jump_pk: id,
    });
    Ok(Effect::new(
        "jumps.logged",
        "logged a jump",
        CommandOutput::Jump(Box::new(jump)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateConnectionJump {
    pub map_id: i64,
    pub jump_pk: i64,
    #[serde(default)]
    #[ts(optional)]
    pub direction: Option<JumpDirection>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub ship_type_id: Option<Option<i64>>,
    /// Explicit mass in kg. Absent while the ship type changes → mass re-derives.
    #[serde(default)]
    #[ts(optional)]
    pub mass: Option<i64>,
}

/// Correct a jump entry. Member+. Works for tracked jumps too (they keep
/// `is_manual = false`, matching legacy).
pub async fn update_jump(
    pool: &PgPool,
    actor: Actor,
    cmd: UpdateConnectionJump,
) -> Result<ConnectionJump> {
    match execute(pool, actor, MapCommand::UpdateConnectionJump(cmd)).await? {
        CommandOutput::Jump(j) => Ok(*j),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_update_jump(
    tx: &mut Tx<'_>,
    cmd: UpdateConnectionJump,
) -> Result<Effect> {
    let current = fetch_jump_tx(tx, cmd.map_id, cmd.jump_pk).await?;
    let Some(connection_id) = current.connection_id else {
        return Err(MapError::NotFound);
    };
    if cmd.mass.is_some_and(|m| m < 0) {
        return Err(MapError::Validation("mass must not be negative".into()));
    }

    let (from_sys, to_sys) = connection_endpoints(tx, cmd.map_id, connection_id).await?;
    let direction = cmd
        .direction
        .unwrap_or(if current.from_solar_system_id == from_sys {
            JumpDirection::Outbound
        } else {
            JumpDirection::Inbound
        });
    let (from, to) = match direction {
        JumpDirection::Outbound => (from_sys, to_sys),
        JumpDirection::Inbound => (to_sys, from_sys),
    };
    let ship_type_id = cmd.ship_type_id.unwrap_or(current.ship_type_id);
    let mass = match cmd.mass {
        Some(m) => m,
        // A changed ship type without an explicit mass re-derives from the hull.
        None if cmd.ship_type_id.is_some() => match ship_type_id {
            Some(t) => ship_mass_tx(tx, t).await?,
            None => current.mass,
        },
        None => current.mass,
    };

    sqlx::query!(
        "update map_connection_jumps
         set from_solar_system_id = $1, to_solar_system_id = $2, ship_type_id = $3,
             mass = $4, updated_at = now()
         where id = $5 and map_id = $6",
        from,
        to,
        ship_type_id,
        mass,
        cmd.jump_pk,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;
    let jump = fetch_jump_tx(tx, cmd.map_id, cmd.jump_pk).await?;
    let was_outbound = current.from_solar_system_id == from_sys;
    let inverse = MapCommand::UpdateConnectionJump(UpdateConnectionJump {
        map_id: cmd.map_id,
        jump_pk: cmd.jump_pk,
        direction: Some(if was_outbound {
            JumpDirection::Outbound
        } else {
            JumpDirection::Inbound
        }),
        ship_type_id: Some(current.ship_type_id),
        mass: Some(current.mass),
    });
    Ok(Effect::new(
        "jumps.updated",
        "edited a jump",
        CommandOutput::Jump(Box::new(jump)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveConnectionJump {
    pub map_id: i64,
    pub jump_pk: i64,
}

/// Delete a jump entry. Member+. Returns the connection it belonged to (for events).
pub async fn remove_jump(
    pool: &PgPool,
    actor: Actor,
    cmd: RemoveConnectionJump,
) -> Result<Option<i64>> {
    match execute(pool, actor, MapCommand::RemoveConnectionJump(cmd)).await? {
        CommandOutput::Count(n) => Ok((n != 0).then_some(n as i64)),
        other => Err(unexpected(other)),
    }
}

pub(super) async fn apply_remove_jump(
    tx: &mut Tx<'_>,
    cmd: RemoveConnectionJump,
) -> Result<Effect> {
    let row = sqlx::query!(
        "delete from map_connection_jumps where id = $1 and map_id = $2
         returning connection_id",
        cmd.jump_pk,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;
    Ok(Effect::new(
        "jumps.removed",
        "removed a jump",
        CommandOutput::Count(row.connection_id.unwrap_or(0) as u64),
    ))
}

/// The latest 10 jumps of a connection, newest first (the counts on the connection are
/// full aggregates). Viewer+.
pub async fn list_jumps(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
    connection_id: i64,
) -> Result<Vec<ConnectionJump>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    let jumps = sqlx::query_as!(
        ConnectionJump,
        r#"select j.id, j.map_id, j.connection_id, j.character_id,
                  c.name as "character_name?", j.ship_type_id, t.name as "ship_type_name?",
                  j.mass, j.is_manual, j.from_solar_system_id, j.to_solar_system_id,
                  j.created_at
           from map_connection_jumps j
           left join characters c on c.id = j.character_id
           left join types t on t.id = j.ship_type_id
           where j.map_id = $1 and j.connection_id = $2
           order by j.id desc limit 10"#,
        map_id,
        connection_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(jumps)
}

/// Read one jump inside the applying transaction, or `NotFound`.
async fn fetch_jump_tx(tx: &mut Tx<'_>, map_id: i64, jump_pk: i64) -> Result<ConnectionJump> {
    sqlx::query_as!(
        ConnectionJump,
        // The `!` overrides restate what the schema already guarantees: sqlx widens
        // the driving table's columns to nullable across this join chain.
        r#"select j.id as "id!", j.map_id as "map_id!", j.connection_id,
                  j.character_id, c.name as "character_name?",
                  j.ship_type_id, t.name as "ship_type_name?",
                  j.mass as "mass!", j.is_manual as "is_manual!",
                  j.from_solar_system_id as "from_solar_system_id!",
                  j.to_solar_system_id as "to_solar_system_id!",
                  j.created_at as "created_at!"
           from map_connection_jumps j
           left join characters c on c.id = j.character_id
           left join types t on t.id = j.ship_type_id
           where j.id = $1 and j.map_id = $2"#,
        jump_pk,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)
}

/// How long a pending (unclaimed) transit waits for its connection to be mapped.
const CLAIM_WINDOW_SECONDS: i32 = 120;
/// How long pending rows live before pruning.
const UNCLAIMED_LIFETIME_MINUTES: i32 = 10;

/// Record an observed transit of a tracked character between two systems. Called from
/// the location poller on every system change; failures must never break polling, so
/// callers log-and-continue.
pub async fn record_transit(
    pool: &PgPool,
    hub: &MapHub,
    character_id: i64,
    from_solar_system_id: i64,
    to_solar_system_id: i64,
) -> Result<()> {
    // Gate travel is never a wormhole jump.
    let is_gate = sqlx::query_scalar!(
        "select exists(
             select 1 from stargates
             where solar_system_id = $1 and destination_system_id = $2
         )",
        from_solar_system_id,
        to_solar_system_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if is_gate {
        return Ok(());
    }

    let character = sqlx::query!(
        r#"select c.user_id, s.ship_type_id, s.ship_name
           from characters c
           left join character_status s on s.character_id = c.id
           where c.id = $1"#,
        character_id,
    )
    .fetch_optional(pool)
    .await?;
    let Some(character) = character else {
        return Ok(());
    };
    let mass = match character.ship_type_id {
        Some(t) => ship_mass(pool, t).await.unwrap_or(0),
        None => 0,
    };

    // Only a character someone signed in with has movements to share; the rest are names
    // we resolved for a killmail.
    let Some(user_id) = character.user_id else {
        return Ok(());
    };

    // Maps where this character's user shares their movements.
    let map_ids = sqlx::query_scalar!(
        "select map_id from map_user_settings where user_id = $1 and tracking_allowed",
        user_id,
    )
    .fetch_all(pool)
    .await?;

    for map_id in map_ids {
        if effective_role(pool, map_id, user_id)
            .await?
            .is_none_or(|r| r < Role::Member)
        {
            continue;
        }

        // Prefer the wormhole edge when a parallel stargate edge exists; a lone
        // stargate edge means this pair is gate travel on this map → skip.
        let connection = sqlx::query!(
            r#"select c.id, c.type as "kind"
               from map_connections c
               join map_solar_systems f on f.id = c.from_system
               join map_solar_systems t on t.id = c.to_system
               where c.map_id = $1
                 and ((f.solar_system_id = $2 and t.solar_system_id = $3)
                      or (f.solar_system_id = $3 and t.solar_system_id = $2))
               order by (c.type = 'wormhole') desc
               limit 1"#,
            map_id,
            from_solar_system_id,
            to_solar_system_id,
        )
        .fetch_optional(pool)
        .await?;

        let connection_id = match &connection {
            Some(c) if c.kind == "wormhole" => Some(c.id),
            Some(_) => continue,
            None => {
                // Unmapped hole: keep a pending row only if the origin is on the map,
                // so a connection created moments later can claim it.
                let placed = sqlx::query_scalar!(
                    "select exists(select 1 from map_solar_systems
                                   where map_id = $1 and solar_system_id = $2)",
                    map_id,
                    from_solar_system_id,
                )
                .fetch_one(pool)
                .await?
                .unwrap_or(false);
                if !placed {
                    continue;
                }
                None
            }
        };

        sqlx::query!(
            "insert into map_connection_jumps
                 (map_id, connection_id, character_id, from_solar_system_id,
                  to_solar_system_id, ship_type_id, ship_name, mass)
             values ($1, $2, $3, $4, $5, $6, $7, $8)",
            map_id,
            connection_id,
            character_id,
            from_solar_system_id,
            to_solar_system_id,
            character.ship_type_id,
            character.ship_name,
            mass,
        )
        .execute(pool)
        .await?;
        if let Some(connection_id) = connection_id {
            hub.publish(MapEvent::ConnectionChanged {
                map_id,
                connection_id,
            });
        }
    }
    Ok(())
}

/// Claim pending transits for a freshly created wormhole connection: rows in the same
/// map matching either direction of the endpoint pair within the claim window.
pub(super) async fn claim_pending_tx(
    tx: &mut Tx<'_>,
    map_id: i64,
    connection_id: i64,
    from_solar_system_id: i64,
    to_solar_system_id: i64,
) -> Result<u64> {
    claim_pending_inner(
        &mut **tx,
        map_id,
        connection_id,
        from_solar_system_id,
        to_solar_system_id,
    )
    .await
}

pub async fn claim_pending(
    pool: &PgPool,
    map_id: i64,
    connection_id: i64,
    from_solar_system_id: i64,
    to_solar_system_id: i64,
) -> Result<u64> {
    claim_pending_inner(
        pool,
        map_id,
        connection_id,
        from_solar_system_id,
        to_solar_system_id,
    )
    .await
}

async fn claim_pending_inner<'e, E>(
    executor: E,
    map_id: i64,
    connection_id: i64,
    from_solar_system_id: i64,
    to_solar_system_id: i64,
) -> Result<u64>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let claimed = sqlx::query!(
        "update map_connection_jumps
         set connection_id = $1, updated_at = now()
         where map_id = $2 and connection_id is null
           and created_at > now() - make_interval(secs => $5)
           and ((from_solar_system_id = $3 and to_solar_system_id = $4)
                or (from_solar_system_id = $4 and to_solar_system_id = $3))",
        connection_id,
        map_id,
        from_solar_system_id,
        to_solar_system_id,
        f64::from(CLAIM_WINDOW_SECONDS),
    )
    .execute(executor)
    .await?
    .rows_affected();
    Ok(claimed)
}

/// Delete pending rows that were never claimed. Claimed jumps live until their
/// connection (or map) is deleted, via the FK cascade.
pub async fn prune_unclaimed(pool: &PgPool) -> Result<u64> {
    let pruned = sqlx::query!(
        "delete from map_connection_jumps
         where connection_id is null
           and created_at < now() - make_interval(mins => $1)",
        UNCLAIMED_LIFETIME_MINUTES,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(pruned)
}

/// Spawn the pending-row prune loop (every 5 minutes).
pub fn start_prune(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(300));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(err) = prune_unclaimed(&pool).await {
                eprintln!("connection jump prune failed: {err}");
            }
        }
    });
}
