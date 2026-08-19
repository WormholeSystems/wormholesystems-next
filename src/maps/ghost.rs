//! Ghost placements: the far side of a wormhole nobody has been through yet.
//!
//! A ghost is an ordinary `map_solar_systems` row with no solar system (see
//! [mapping.md](../../docs/database/mapping.md#ghost-placements)), so it draws, moves,
//! aliases and connects like any other node. What it cannot do is anything that needs a
//! system: hold signatures or intel, be routed through, be a waypoint.
//!
//! Resolving one is [`resolve_ghost_system`]. When the system turns out to be on the map
//! already — the hole led back into the chain — the ghost is merged into that placement
//! rather than duplicating it, which is also what the jump tracker needs when someone
//! finally flies the hole.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::command::{CommandOutput, Effect, MapCommand, Sequence, Tx, execute};
use super::connection::AddConnection;
use super::error::{MapError, Result};
use super::solar_system::{MapSolarSystem, unexpected};
use super::{Actor, ConnectionType, WormholeSize};

/// Node width in world px, and the clear space kept between nodes, both mirroring
/// `frontend/src/lib/map/helpers.ts`. The client owns layout for everything it anchors on
/// a viewport; these exist so a node the *server* places lands on the same lattice.
const NODE_WIDTH: f64 = 180.0;
const NODE_GAP_CELLS: f64 = 1.0;

/// The first free slot beside `base`, then down that column — the client's `freePosition`,
/// for placements made inside a command. Siblings stack under the first one rather than
/// marching right across the map.
fn free_position(placed: &[(f64, f64)], base: (f64, f64)) -> (f64, f64) {
    let grid = super::grid();
    let node_h = 2.0 * grid.cell_size;
    let gap = NODE_GAP_CELLS * grid.cell_size;
    let step_x = NODE_WIDTH + gap;
    let step_y = node_h + gap;
    let max_x = grid.world_width - NODE_WIDTH;
    let max_y = grid.world_height - node_h;
    let snap = |v: f64| (v / grid.cell_size).round() * grid.cell_size;
    let crowded = |x: f64, y: f64| {
        placed
            .iter()
            .any(|(px, py)| (x - px).abs() < step_x && (y - py).abs() < step_y)
    };

    let bx = snap(base.0.clamp(0.0, max_x));
    let by = snap(base.1.clamp(0.0, max_y));
    if !crowded(bx, by) {
        return (bx, by);
    }
    let mut column = 1.0;
    loop {
        let x = snap(bx + column * step_x);
        if x > max_x {
            return (bx, by);
        }
        let mut row = 0.0;
        loop {
            let y = snap(by + row * step_y);
            if y > max_y {
                break;
            }
            if !crowded(x, y) {
                return (x, y);
            }
            row += 1.0;
        }
        column += 1.0;
    }
}

/// Raise a ghost for every wormhole scanned in this system that is not on the map yet,
/// when the map is set up for it. Returns the placements raised, for the caller's undo.
///
/// Runs inside the signature write that made the hole known — a pasted scan, a row typed
/// in by hand, a signature recategorised as a wormhole — so the nodes and the scan land in
/// one transaction, as one entry in the history, whichever client did the writing.
pub(super) async fn ghost_unmapped_holes(
    tx: &mut Tx<'_>,
    map_id: i64,
    solar_system_id: i64,
) -> Result<Vec<i64>> {
    let wanted = sqlx::query_scalar!(
        "select ghost_unlinked_wormholes from maps where id = $1",
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(false);
    if !wanted {
        return Ok(Vec::new());
    }

    let from = sqlx::query!(
        "select id, position_x, position_y from map_solar_systems
         where map_id = $1 and solar_system_id = $2",
        map_id,
        solar_system_id,
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some(from) = from else {
        return Ok(Vec::new());
    };

    let holes = sqlx::query!(
        r#"select id, size as "size: WormholeSize" from signatures
           where map_id = $1 and solar_system_id = $2
             and "group" = 'wormhole' and connection_id is null
           order by signature_id"#,
        map_id,
        solar_system_id,
    )
    .fetch_all(&mut **tx)
    .await?;
    if holes.is_empty() {
        return Ok(Vec::new());
    }

    let mut placed: Vec<(f64, f64)> = sqlx::query!(
        "select position_x, position_y from map_solar_systems where map_id = $1",
        map_id,
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|r| (r.position_x, r.position_y))
    .collect();

    let mut raised = Vec::new();
    for hole in holes {
        let (x, y) = free_position(&placed, (from.position_x, from.position_y));
        placed.push((x, y));
        let effect = apply_add_ghost_system(
            tx,
            AddGhostSystem {
                map_id,
                from_system: from.id,
                signature_pk: Some(hole.id),
                x,
                y,
                alias: None,
                size: hole.size,
            },
        )
        .await?;
        let CommandOutput::System(ghost) = effect.output else {
            return Err(unexpected(effect.output));
        };
        raised.push(ghost.id);
    }
    Ok(raised)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddGhostSystem {
    pub map_id: i64,
    /// The placement the signature was scanned in. The ghost hangs off it.
    pub from_system: i64,
    /// The wormhole signature this hole is, so the two stay in lock-step from the start.
    #[serde(default)]
    pub signature_pk: Option<i64>,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub size: Option<WormholeSize>,
}

/// Put the far side of a wormhole signature on the map, before anyone knows what it is.
/// Member+. Reached through the signature writes ([`ghost_unmapped_holes`]) rather than
/// directly: a hole exists because a scan says so.
pub async fn add_ghost_system(
    pool: &PgPool,
    actor: Actor,
    cmd: AddGhostSystem,
) -> Result<MapSolarSystem> {
    execute(pool, actor, MapCommand::AddGhostSystem(cmd))
        .await?
        .system()
}

pub(super) async fn apply_add_ghost_system(tx: &mut Tx<'_>, cmd: AddGhostSystem) -> Result<Effect> {
    let source = sqlx::query!(
        "select solar_system_id from map_solar_systems where id = $1 and map_id = $2",
        cmd.from_system,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;

    // The signature has to be one scanned in the source system, and still unlinked: a
    // second ghost for a hole already on the map is exactly the duplicate this avoids.
    let before = match cmd.signature_pk {
        Some(pk) => {
            let sig = sqlx::query!(
                "select solar_system_id, connection_id from signatures where id = $1 and map_id = $2",
                pk,
                cmd.map_id,
            )
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(MapError::NotFound)?;
            if Some(sig.solar_system_id) != source.solar_system_id {
                return Err(MapError::Validation(
                    "that signature was scanned in another system".into(),
                ));
            }
            if sig.connection_id.is_some() {
                return Err(MapError::Conflict(
                    "that signature is already on the map".into(),
                ));
            }
            Some(super::tracking::signature_state(tx, cmd.map_id, pk).await?)
        }
        None => None,
    };

    let ghost = sqlx::query_as!(
        MapSolarSystem,
        "insert into map_solar_systems (map_id, solar_system_id, position_x, position_y, alias)
         values ($1, null, $2, $3, $4)
         returning id, map_id, solar_system_id, position_x, position_y, alias, created_at",
        cmd.map_id,
        cmd.x,
        cmd.y,
        cmd.alias.as_deref(),
    )
    .fetch_one(&mut **tx)
    .await?;

    let effect = super::connection::apply_add_connection(
        tx,
        AddConnection {
            map_id: cmd.map_id,
            from_system: cmd.from_system,
            to_system: ghost.id,
            kind: ConnectionType::Wormhole,
            size: cmd.size,
        },
    )
    .await?;
    let CommandOutput::Connection(connection) = effect.output else {
        return Err(unexpected(effect.output));
    };

    if let Some(pk) = cmd.signature_pk {
        super::tracking::link(tx, cmd.map_id, pk, connection.id).await?;
    }

    let mut steps = super::tracking::undo_signature(cmd.map_id, before.as_ref());
    steps.push(MapCommand::RemoveRestored(
        super::solar_system::RemoveRestored {
            map_id: cmd.map_id,
            system_ids: vec![ghost.id],
            connection_ids: vec![connection.id],
        },
    ));

    Ok(Effect::new(
        "systems.added",
        "put an unscanned hole on the map",
        CommandOutput::System(Box::new(ghost)),
    )
    .undo_with(MapCommand::Sequence(Sequence {
        map_id: cmd.map_id,
        steps,
    })))
}

/// Say which system a ghost turned out to be. `solar_system_id: None` takes it back to a
/// ghost, and exists as the inverse of the first form rather than as an action of its own.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ResolveGhostSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    #[serde(default)]
    #[ts(optional)]
    pub solar_system_id: Option<i64>,
}

/// Assign a system to a ghost, merging into the existing placement if the map already has
/// that system. Member+.
pub async fn resolve_ghost_system(
    pool: &PgPool,
    actor: Actor,
    cmd: ResolveGhostSystem,
) -> Result<MapSolarSystem> {
    execute(pool, actor, MapCommand::ResolveGhostSystem(cmd))
        .await?
        .system()
}

pub(super) async fn apply_resolve_ghost_system(
    tx: &mut Tx<'_>,
    cmd: ResolveGhostSystem,
) -> Result<Effect> {
    let placement = sqlx::query!(
        "select solar_system_id, position_x, position_y, alias
         from map_solar_systems where id = $1 and map_id = $2",
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;

    let Some(solar_system_id) = cmd.solar_system_id else {
        return unresolve(
            tx,
            cmd.map_id,
            cmd.map_solar_system_id,
            placement.solar_system_id,
        )
        .await;
    };

    if placement.solar_system_id.is_some() {
        return Err(MapError::Validation("that node is already a system".into()));
    }

    let name = sqlx::query_scalar!(
        "select name from solar_systems where id = $1",
        solar_system_id
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| MapError::Validation(format!("unknown solar system {solar_system_id}")))?;

    let existing = sqlx::query_scalar!(
        "select id from map_solar_systems where map_id = $1 and solar_system_id = $2",
        cmd.map_id,
        solar_system_id,
    )
    .fetch_optional(&mut **tx)
    .await?;

    match existing {
        Some(target) => merge(tx, cmd.map_id, cmd.map_solar_system_id, target, &name).await,
        None => {
            let placed = sqlx::query_as!(
                MapSolarSystem,
                "update map_solar_systems set solar_system_id = $1 where id = $2 and map_id = $3
                 returning id, map_id, solar_system_id, position_x, position_y, alias, created_at",
                solar_system_id,
                cmd.map_solar_system_id,
                cmd.map_id,
            )
            .fetch_one(&mut **tx)
            .await?;
            claim_pending_for(tx, cmd.map_id, cmd.map_solar_system_id, solar_system_id).await?;
            Ok(Effect::new(
                "systems.details",
                format!("scanned that hole as {name}"),
                CommandOutput::System(Box::new(placed)),
            )
            .undo_with(MapCommand::ResolveGhostSystem(ResolveGhostSystem {
                map_id: cmd.map_id,
                map_solar_system_id: cmd.map_solar_system_id,
                solar_system_id: None,
            })))
        }
    }
}

/// Back to a ghost. Refuses once the system holds a scan, because those rows hang off the
/// system id and would be cascaded away by clearing it.
async fn unresolve(
    tx: &mut Tx<'_>,
    map_id: i64,
    map_solar_system_id: i64,
    current: Option<i64>,
) -> Result<Effect> {
    let Some(current) = current else {
        return Err(MapError::Validation("that node is already a ghost".into()));
    };
    let signatures = sqlx::query_scalar!(
        "select count(*) from signatures where map_id = $1 and solar_system_id = $2",
        map_id,
        current,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(0);
    if signatures > 0 {
        return Err(MapError::Conflict(
            "that system has signatures now; remove them first".into(),
        ));
    }

    let placed = sqlx::query_as!(
        MapSolarSystem,
        "update map_solar_systems
         set solar_system_id = null, is_home = false, is_rally = false
         where id = $1 and map_id = $2
         returning id, map_id, solar_system_id, position_x, position_y, alias, created_at",
        map_solar_system_id,
        map_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    Ok(Effect::new(
        "systems.details",
        "took that system back off the map",
        CommandOutput::System(Box::new(placed)),
    )
    .undo_with(MapCommand::ResolveGhostSystem(ResolveGhostSystem {
        map_id,
        map_solar_system_id,
        solar_system_id: Some(current),
    })))
}

/// The ghost turned out to be a system already on the map: hand its edges over and drop
/// it, rather than placing the same system twice.
async fn merge(
    tx: &mut Tx<'_>,
    map_id: i64,
    ghost_id: i64,
    target: i64,
    name: &str,
) -> Result<Effect> {
    let loops = sqlx::query_scalar!(
        "select count(*) from map_connections
         where map_id = $1
           and ((from_system = $2 and to_system = $3) or (from_system = $3 and to_system = $2))",
        map_id,
        ghost_id,
        target,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(0);
    if loops > 0 {
        return Err(MapError::Conflict(format!(
            "that hole already connects to {name}, so it cannot lead there"
        )));
    }

    let from_ids: Vec<i64> = sqlx::query_scalar!(
        "update map_connections set from_system = $1, updated_at = now()
         where map_id = $2 and from_system = $3 returning id",
        target,
        map_id,
        ghost_id,
    )
    .fetch_all(&mut **tx)
    .await?;
    let to_ids: Vec<i64> = sqlx::query_scalar!(
        "update map_connections set to_system = $1, updated_at = now()
         where map_id = $2 and to_system = $3 returning id",
        target,
        map_id,
        ghost_id,
    )
    .fetch_all(&mut **tx)
    .await?;

    let ghost = sqlx::query!(
        "delete from map_solar_systems where id = $1 and map_id = $2
         returning position_x, position_y, alias",
        ghost_id,
        map_id,
    )
    .fetch_one(&mut **tx)
    .await?;

    let placed = sqlx::query_as!(
        MapSolarSystem,
        "select id, map_id, solar_system_id, position_x, position_y, alias, created_at
         from map_solar_systems where id = $1",
        target,
    )
    .fetch_one(&mut **tx)
    .await?;
    if let Some(system) = placed.solar_system_id {
        claim_pending_for(tx, map_id, target, system).await?;
    }

    Ok(Effect::new(
        "systems.details",
        format!("scanned that hole as {name}, already on the map"),
        CommandOutput::System(Box::new(placed)),
    )
    .undo_with(MapCommand::RestoreGhostSystem(RestoreGhostSystem {
        map_id,
        id: ghost_id,
        position_x: ghost.position_x,
        position_y: ghost.position_y,
        alias: ghost.alias,
        from_connection_ids: from_ids,
        to_connection_ids: to_ids,
    })))
}

/// Undo of a merge: the ghost row comes back with its own id, and the edges that were
/// handed over point at it again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreGhostSystem {
    pub map_id: i64,
    pub id: i64,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub from_connection_ids: Vec<i64>,
    pub to_connection_ids: Vec<i64>,
}

pub(super) async fn apply_restore_ghost_system(
    tx: &mut Tx<'_>,
    cmd: RestoreGhostSystem,
) -> Result<Effect> {
    sqlx::query!(
        "insert into map_solar_systems (id, map_id, solar_system_id, position_x, position_y, alias)
         overriding system value
         values ($1, $2, null, $3, $4, $5)
         on conflict (id) do nothing",
        cmd.id,
        cmd.map_id,
        cmd.position_x,
        cmd.position_y,
        cmd.alias.as_deref(),
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        "update map_connections set from_system = $1 where map_id = $2 and id = any($3)",
        cmd.id,
        cmd.map_id,
        &cmd.from_connection_ids,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        "update map_connections set to_system = $1 where map_id = $2 and id = any($3)",
        cmd.id,
        cmd.map_id,
        &cmd.to_connection_ids,
    )
    .execute(&mut **tx)
    .await?;

    let ghost = sqlx::query_as!(
        MapSolarSystem,
        "select id, map_id, solar_system_id, position_x, position_y, alias, created_at
         from map_solar_systems where id = $1",
        cmd.id,
    )
    .fetch_one(&mut **tx)
    .await?;

    let inverse = MapCommand::ResolveGhostSystem(ResolveGhostSystem {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.id,
        solar_system_id: None,
    });
    Ok(Effect::new(
        "systems.added",
        "put the unscanned hole back",
        CommandOutput::System(Box::new(ghost)),
    )
    .undo_with(inverse))
}

/// The ghosts that removing these placements or connections would strand.
///
/// A ghost is the far side of a wormhole and nothing else, so one with no connection left
/// is not a place at all and goes with whatever it hung off. Real systems are left alone
/// even when they end up edgeless: somebody put those on the map on purpose.
///
/// Asked before the deletion rather than after, so the caller can fold the answer into the
/// snapshot it takes and undo brings the whole thing back in one step.
pub(super) async fn stranded_ghosts(
    tx: &mut Tx<'_>,
    map_id: i64,
    removed_systems: &[i64],
    removed_connections: &[i64],
) -> Result<Vec<i64>> {
    Ok(sqlx::query_scalar!(
        r#"select g.id as "id!" from map_solar_systems g
           where g.map_id = $1
             and g.solar_system_id is null
             and not g.is_home and not g.is_pinned and not g.is_rally
             and exists (
                 select 1 from map_connections c
                 where c.map_id = $1 and (c.from_system = g.id or c.to_system = g.id)
             )
             and not exists (
                 select 1 from map_connections c
                 where c.map_id = $1
                   and (c.from_system = g.id or c.to_system = g.id)
                   and c.id <> all($3)
                   and c.from_system <> all($2)
                   and c.to_system <> all($2)
             )"#,
        map_id,
        removed_systems,
        removed_connections,
    )
    .fetch_all(&mut **tx)
    .await?)
}

/// A hole is often flown before it is named. Once the far side is known, transits that
/// were recorded without a connection can be claimed by the edges of this placement.
async fn claim_pending_for(
    tx: &mut Tx<'_>,
    map_id: i64,
    placement_id: i64,
    solar_system_id: i64,
) -> Result<()> {
    let edges = sqlx::query!(
        r#"select c.id,
                  case when c.from_system = $2 then t.solar_system_id
                       else f.solar_system_id end as "other?"
           from map_connections c
           join map_solar_systems f on f.id = c.from_system
           join map_solar_systems t on t.id = c.to_system
           where c.map_id = $1 and (c.from_system = $2 or c.to_system = $2)
             and c.type = 'wormhole'"#,
        map_id,
        placement_id,
    )
    .fetch_all(&mut **tx)
    .await?;
    for edge in edges {
        let Some(other) = edge.other else { continue };
        super::jumps::claim_pending_tx(tx, map_id, edge.id, solar_system_id, other).await?;
    }
    Ok(())
}
