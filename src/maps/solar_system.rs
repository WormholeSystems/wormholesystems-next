//! Placed solar systems: placing, moving, removing, and aliasing systems on a map. All
//! are Member+ (see [access.md](../../docs/database/access.md)).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use sqlx::PgPool;

use super::access::require_role;
use super::command::{CommandOutput, Effect, MapCommand, Tx, execute};
use super::error::{MapError, Result};
use super::{Actor, ConnectionType, MassStatus, Role, SignatureGroup, TimeStatus, WormholeSize};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapSolarSystem {
    pub id: i64,
    pub map_id: i64,
    /// `None` for a ghost: the far side of a wormhole nobody has been through yet.
    pub solar_system_id: Option<i64>,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A static wormhole a system always has, plus the class it leads to (`dest_class` is the
/// `wormhole_class_id` encoding; `None` for the few codes with no fixed destination) and
/// the hole physics for the static tooltip.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Static {
    pub code: String,
    pub dest_class: Option<i32>,
    /// Total mass the hole can pass before collapsing, kg.
    pub total_mass: Option<i64>,
    /// Max mass of a single ship per jump, kg.
    pub max_jump_mass: Option<i64>,
    pub lifetime_hours: Option<f64>,
    /// Scan signature strength in percent (higher = easier to scan).
    pub signature_strength: Option<f64>,
}

/// One buff/debuff a wormhole effect applies, for the node's effect popover. `kind` is the
/// effect strength tier; `stat` is what it modifies; `value` is the (already-formatted) amount.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct EffectModifier {
    pub kind: String,
    pub stat: String,
    pub value: String,
}

/// The modifiers a wormhole effect applies at a given class. Reference data, no auth needed.
/// The modifier table is keyed by class 1..6; special classes map to the strength tier the
/// game uses for them (C13 has C6-strength effects, drifter systems C14-18 have C2 strength).
pub async fn effect_modifiers(
    pool: &PgPool,
    effect_name: &str,
    wormhole_class_id: i32,
) -> Result<Vec<EffectModifier>> {
    let effective_class = match wormhole_class_id {
        13 => 6,
        14..=18 => 2,
        c => c,
    };
    let rows = sqlx::query_as!(
        EffectModifier,
        "select kind, stat, value from wormhole_effect_modifiers
         where effect_name = $1 and wormhole_class_id = $2
         order by stat, kind",
        effect_name,
        effective_class,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Who holds sovereignty in a system. The variant *is* the holder kind, so the node knows
/// which EVE image endpoint to use for the icon; only alliances/corps carry a ticker.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Sovereignty {
    Alliance {
        id: i64,
        name: String,
        ticker: String,
    },
    Corporation {
        id: i64,
        name: String,
        ticker: String,
    },
    Faction {
        id: i64,
        name: String,
    },
}

/// A placed system enriched with everything a map node displays. Read-only, built by
/// `get_map` from joins across the SDE + intel + sovereignty tables. Mutations use the lean
/// [`MapSolarSystem`].
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapSystemView {
    // Placement (map_solar_systems).
    pub id: i64,
    pub map_id: i64,
    /// `None` for a ghost, which is what makes every reference field below optional too:
    /// there is no system yet to look them up from.
    pub solar_system_id: Option<i64>,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub is_home: bool,
    pub is_rally: bool,
    pub is_pinned: bool,
    // Intel (map_solar_system_details; defaults when no row exists yet).
    pub status: super::SystemStatus,
    pub occupying_group: Option<String>,
    // Reference (solar_systems / regions / constellations). All `None` on a ghost.
    pub name: Option<String>,
    pub security_status: Option<f64>,
    pub wormhole_class_id: Option<i32>,
    pub region: Option<String>,
    pub region_id: Option<i64>,
    pub constellation_id: Option<i64>,
    pub constellation: Option<String>,
    // Wormhole reference (wormhole_systems / statics).
    pub effect_name: Option<String>,
    pub is_shattered: bool,
    /// Kill-activity threat (wormhole systems only; `None` for k-space).
    pub threat_level: Option<super::ThreatLevel>,
    pub statics: Vec<Static>,
    // Sovereignty (system_sovereignty → alliance/corp/faction).
    pub sovereignty: Option<Sovereignty>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
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
    execute(pool, actor, MapCommand::AddSystem(cmd))
        .await?
        .system()
}

pub(super) async fn apply_add_system(tx: &mut Tx<'_>, cmd: AddSystem) -> Result<Effect> {
    let name = sqlx::query_scalar!(
        "select name from solar_systems where id = $1",
        cmd.solar_system_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| MapError::Validation(format!("unknown solar system {}", cmd.solar_system_id)))?;

    let already = sqlx::query_scalar!(
        "select exists(select 1 from map_solar_systems where map_id = $1 and solar_system_id = $2)",
        cmd.map_id,
        cmd.solar_system_id,
    )
    .fetch_one(&mut **tx)
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
    .fetch_one(&mut **tx)
    .await?;

    let inverse = MapCommand::RemoveSystems(RemoveSystems {
        map_id: cmd.map_id,
        map_solar_system_ids: vec![placed.id],
    });
    Ok(Effect::new(
        "systems.added",
        format!("added {name}"),
        CommandOutput::System(Box::new(placed)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
}

/// Remove a system from a map. Cascades the system's signatures and any connections it
/// is an endpoint of; its persisted details survive.
pub async fn remove_system(pool: &PgPool, actor: Actor, cmd: RemoveSystem) -> Result<()> {
    execute(pool, actor, MapCommand::RemoveSystem(cmd)).await?;
    Ok(())
}

/// An effect that changed nothing. No inverse, so it stays out of the undo history: pressing
/// delete on a pinned system should not leave a step that undoes to nothing.
fn kept(kind: &'static str, label: &'static str) -> Effect {
    Effect::new(kind, label, CommandOutput::Count(0)).entries(0)
}

pub(super) async fn apply_remove_system(tx: &mut Tx<'_>, cmd: RemoveSystem) -> Result<Effect> {
    let protected = sqlx::query_scalar!(
        r#"select (is_home or is_pinned) as "protected!" from map_solar_systems
           where id = $1 and map_id = $2"#,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;
    if protected {
        return Ok(kept("systems.removed", "kept a protected system"));
    }

    // The unmapped holes hanging off it go with it, and into the same snapshot, so one undo
    // brings both back.
    let mut ids = vec![cmd.map_solar_system_id];
    ids.extend(super::ghost::stranded_ghosts(tx, cmd.map_id, &ids, &[]).await?);

    let snapshot = capture_systems(tx, cmd.map_id, &ids).await?;
    remove_captured_signatures(tx, cmd.map_id, &snapshot).await?;
    // The guard is repeated on the delete itself, so the rule travels with the query.
    let deleted = sqlx::query!(
        "delete from map_solar_systems
         where id = any($1) and map_id = $2 and not is_home and not is_pinned",
        &ids,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if deleted == 0 {
        return Err(MapError::NotFound);
    }
    let label = format!("removed {}", snapshot.label());
    Ok(Effect::new("systems.removed", label, CommandOutput::None)
        .undo_with(MapCommand::RestoreSystems(snapshot)))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveSystems {
    pub map_id: i64,
    pub map_solar_system_ids: Vec<i64>,
}

/// Remove several placed systems at once (multi-select delete). Same cascade as
/// [`remove_system`]. Returns the number actually removed; an empty id list is a no-op.
pub async fn remove_systems(pool: &PgPool, actor: Actor, cmd: RemoveSystems) -> Result<u64> {
    execute(pool, actor, MapCommand::RemoveSystems(cmd))
        .await?
        .count()
}

pub(super) async fn apply_remove_systems(tx: &mut Tx<'_>, cmd: RemoveSystems) -> Result<Effect> {
    // Home and pinned systems are deliberate markers that every sweep passes over, so a
    // marquee across the chain must not take them either.
    let rows = sqlx::query!(
        r#"select id, (is_home or is_pinned) as "protected!"
           from map_solar_systems where map_id = $1 and id = any($2)"#,
        cmd.map_id,
        &cmd.map_solar_system_ids,
    )
    .fetch_all(&mut **tx)
    .await?;
    let removable: Vec<i64> = rows.iter().filter(|r| !r.protected).map(|r| r.id).collect();
    let held = rows.len() - removable.len();

    if removable.is_empty() {
        // A protected system is passed over rather than failing the whole delete.
        return Ok(kept("systems.removed", "kept the protected systems"));
    }

    // Same as the single removal: the unmapped holes these leave behind come too.
    let mut removable = removable;
    removable.extend(super::ghost::stranded_ghosts(tx, cmd.map_id, &removable, &[]).await?);

    let snapshot = capture_systems(tx, cmd.map_id, &removable).await?;
    remove_captured_signatures(tx, cmd.map_id, &snapshot).await?;
    // The guard is repeated on the delete itself, so the rule travels with the query.
    let deleted = sqlx::query!(
        "delete from map_solar_systems
         where map_id = $1 and id = any($2) and not is_home and not is_pinned",
        cmd.map_id,
        &removable,
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();

    let label = match held {
        0 => format!("removed {}", snapshot.label()),
        n => format!("removed {} (kept {n} protected)", snapshot.label()),
    };
    Ok(
        Effect::new("systems.removed", label, CommandOutput::Count(deleted))
            .entries(deleted as i64)
            .undo_with(MapCommand::RestoreSystems(snapshot)),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ClearMap {
    pub map_id: i64,
}

/// Remove every placed system on a map except the home system and any pinned systems.
/// Connections to removed systems cascade. Returns the number removed.
pub async fn clear_map(pool: &PgPool, actor: Actor, cmd: ClearMap) -> Result<u64> {
    execute(pool, actor, MapCommand::ClearMap(cmd))
        .await?
        .count()
}

pub(super) async fn apply_clear_map(tx: &mut Tx<'_>, cmd: ClearMap) -> Result<Effect> {
    let doomed: Vec<i64> = sqlx::query_scalar!(
        "select id from map_solar_systems
         where map_id = $1 and not is_home and not is_pinned",
        cmd.map_id,
    )
    .fetch_all(&mut **tx)
    .await?;
    let snapshot = capture_systems(tx, cmd.map_id, &doomed).await?;
    let deleted = sqlx::query!(
        "delete from map_solar_systems where map_id = $1 and not is_home and not is_pinned",
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok(Effect::new(
        "map.cleared",
        format!("cleared the map ({deleted} systems)"),
        CommandOutput::Count(deleted),
    )
    .entries(deleted as i64)
    .undo_with(MapCommand::RestoreSystems(snapshot)))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MoveSystem {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub x: f64,
    pub y: f64,
}

/// Move a placed system to a new position.
pub async fn move_system(pool: &PgPool, actor: Actor, cmd: MoveSystem) -> Result<()> {
    execute(pool, actor, MapCommand::MoveSystem(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_move_system(tx: &mut Tx<'_>, cmd: MoveSystem) -> Result<Effect> {
    let before = sqlx::query!(
        "select position_x, position_y, is_pinned from map_solar_systems
         where id = $1 and map_id = $2",
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;
    // The server holds the drag lock too, rather than trusting the client. Home is not
    // pinned by being home: it can be dragged around like anything else.
    if before.is_pinned {
        return Ok(kept("systems.moved", "a pinned system stayed put"));
    }
    sqlx::query!(
        "update map_solar_systems set position_x = $1, position_y = $2
         where id = $3 and map_id = $4 and not is_pinned",
        cmd.x,
        cmd.y,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;
    let inverse = MapCommand::MoveSystem(MoveSystem {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        x: before.position_x,
        y: before.position_y,
    });
    Ok(Effect::new("systems.moved", "moved a system", CommandOutput::None).undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SystemMove {
    pub map_solar_system_id: i64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MoveSystems {
    pub map_id: i64,
    pub moves: Vec<SystemMove>,
}

/// Move several placed systems at once (multi-drag), in one transaction.
pub async fn move_systems(pool: &PgPool, actor: Actor, cmd: MoveSystems) -> Result<()> {
    execute(pool, actor, MapCommand::MoveSystems(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_move_systems(tx: &mut Tx<'_>, cmd: MoveSystems) -> Result<Effect> {
    let ids: Vec<i64> = cmd.moves.iter().map(|m| m.map_solar_system_id).collect();
    // Only the ones that will actually move, so undo puts back exactly what shifted.
    let before: Vec<SystemMove> = sqlx::query!(
        "select id, position_x, position_y from map_solar_systems
         where map_id = $1 and id = any($2) and not is_pinned",
        cmd.map_id,
        &ids,
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|r| SystemMove {
        map_solar_system_id: r.id,
        x: r.position_x,
        y: r.position_y,
    })
    .collect();

    if before.is_empty() {
        return Ok(kept("systems.moved", "the pinned systems stayed put"));
    }

    let movable: std::collections::HashSet<i64> =
        before.iter().map(|m| m.map_solar_system_id).collect();
    for m in cmd
        .moves
        .iter()
        .filter(|m| movable.contains(&m.map_solar_system_id))
    {
        sqlx::query!(
            "update map_solar_systems set position_x = $1, position_y = $2
             where id = $3 and map_id = $4 and not is_pinned",
            m.x,
            m.y,
            m.map_solar_system_id,
            cmd.map_id,
        )
        .execute(&mut **tx)
        .await?;
    }
    let count = movable.len();
    let label = if count == 1 {
        "moved a system".to_string()
    } else {
        format!("moved {count} systems")
    };
    let inverse = MapCommand::MoveSystems(MoveSystems {
        map_id: cmd.map_id,
        moves: before,
    });
    Ok(Effect::new("systems.moved", label, CommandOutput::None)
        .entries(count as i64)
        .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetAlias {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub alias: Option<String>,
}

/// Set or clear a placement's ephemeral alias.
pub async fn set_alias(pool: &PgPool, actor: Actor, cmd: SetAlias) -> Result<()> {
    execute(pool, actor, MapCommand::SetAlias(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_set_alias(tx: &mut Tx<'_>, cmd: SetAlias) -> Result<Effect> {
    let before = sqlx::query_scalar!(
        "select alias from map_solar_systems where id = $1 and map_id = $2",
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;
    sqlx::query!(
        "update map_solar_systems set alias = $1 where id = $2 and map_id = $3",
        cmd.alias.as_deref(),
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;
    let label = match &cmd.alias {
        Some(alias) => format!("renamed a system to {alias}"),
        None => "cleared a system alias".to_string(),
    };
    let inverse = MapCommand::SetAlias(SetAlias {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        alias: before,
    });
    Ok(Effect::new("systems.aliased", label, CommandOutput::None).undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetStatus {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub status: super::SystemStatus,
}

/// Set a placed system's intel status. Upserts the persisted details row, keyed by the
/// placement's `(map_id, solar_system_id)`.
pub async fn set_status(pool: &PgPool, actor: Actor, cmd: SetStatus) -> Result<()> {
    execute(pool, actor, MapCommand::SetStatus(cmd)).await?;
    Ok(())
}

/// The system behind a placement, or a plain refusal when it is still a ghost. Intel is
/// keyed by system rather than by placement, so there is nowhere to put it yet.
async fn system_behind(tx: &mut Tx<'_>, map_id: i64, map_solar_system_id: i64) -> Result<i64> {
    sqlx::query_scalar!(
        "select solar_system_id from map_solar_systems where id = $1 and map_id = $2",
        map_solar_system_id,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?
    .ok_or_else(|| {
        MapError::Validation("assign a system to that hole before recording intel on it".into())
    })
}

pub(super) async fn apply_set_status(tx: &mut Tx<'_>, cmd: SetStatus) -> Result<Effect> {
    system_behind(tx, cmd.map_id, cmd.map_solar_system_id).await?;
    let before = sqlx::query_scalar!(
        r#"select coalesce(d.status, 'unknown') as "status!: super::SystemStatus"
           from map_solar_systems mss
           left join map_solar_system_details d
               on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
           where mss.id = $1 and mss.map_id = $2"#,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;

    sqlx::query!(
        "insert into map_solar_system_details (map_id, solar_system_id, status)
         select map_id, solar_system_id, $3 from map_solar_systems where id = $2 and map_id = $1
         on conflict (map_id, solar_system_id)
             do update set status = excluded.status, updated_at = now()",
        cmd.map_id,
        cmd.map_solar_system_id,
        cmd.status.as_str(),
    )
    .execute(&mut **tx)
    .await?;
    let inverse = MapCommand::SetStatus(SetStatus {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        status: before,
    });
    Ok(Effect::new(
        "systems.status",
        format!("set a system to {}", cmd.status.as_str()),
        CommandOutput::None,
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetOccupier {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub occupier: Option<String>,
}

/// Set or clear who occupies a placed system (free text, like the alias). Upserts the
/// persisted details row.
pub async fn set_occupier(pool: &PgPool, actor: Actor, cmd: SetOccupier) -> Result<()> {
    execute(pool, actor, MapCommand::SetOccupier(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_set_occupier(tx: &mut Tx<'_>, cmd: SetOccupier) -> Result<Effect> {
    system_behind(tx, cmd.map_id, cmd.map_solar_system_id).await?;
    let before = detail_text(
        tx,
        cmd.map_id,
        cmd.map_solar_system_id,
        DetailColumn::Occupier,
    )
    .await?;
    sqlx::query!(
        "insert into map_solar_system_details (map_id, solar_system_id, occupying_group)
         select map_id, solar_system_id, $3 from map_solar_systems where id = $2 and map_id = $1
         on conflict (map_id, solar_system_id)
             do update set occupying_group = excluded.occupying_group, updated_at = now()",
        cmd.map_id,
        cmd.map_solar_system_id,
        cmd.occupier.as_deref(),
    )
    .execute(&mut **tx)
    .await?;
    let label = match &cmd.occupier {
        Some(occupier) => format!("set the occupier to {occupier}"),
        None => "cleared an occupier".to_string(),
    };
    let inverse = MapCommand::SetOccupier(SetOccupier {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        occupier: before,
    });
    Ok(Effect::new("systems.occupier", label, CommandOutput::None).undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetNotes {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub notes: Option<String>,
}

/// Set or clear a placed system's notes (markdown free text). Member+ only, like every
/// other intel write; upserts the persisted details row.
pub async fn set_notes(pool: &PgPool, actor: Actor, cmd: SetNotes) -> Result<()> {
    execute(pool, actor, MapCommand::SetNotes(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_set_notes(tx: &mut Tx<'_>, cmd: SetNotes) -> Result<Effect> {
    system_behind(tx, cmd.map_id, cmd.map_solar_system_id).await?;
    let before = detail_text(tx, cmd.map_id, cmd.map_solar_system_id, DetailColumn::Notes).await?;
    sqlx::query!(
        "insert into map_solar_system_details (map_id, solar_system_id, notes)
         select map_id, solar_system_id, $3 from map_solar_systems where id = $2 and map_id = $1
         on conflict (map_id, solar_system_id)
             do update set notes = excluded.notes, updated_at = now()",
        cmd.map_id,
        cmd.map_solar_system_id,
        cmd.notes.as_deref(),
    )
    .execute(&mut **tx)
    .await?;
    let inverse = MapCommand::SetNotes(SetNotes {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        notes: before,
    });
    Ok(Effect::new("systems.notes", "edited notes", CommandOutput::None).undo_with(inverse))
}

/// A placed system's member-gated intel details (currently just the notes). Viewers never
/// receive this: the read itself requires Member.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SystemDetails {
    pub notes: Option<String>,
}

/// The member-gated details for a placement. `Forbidden` for viewers.
pub async fn system_details(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
    map_solar_system_id: i64,
) -> Result<SystemDetails> {
    require_role(pool, map_id, actor.user_id, Role::Member).await?;
    let row = sqlx::query!(
        "select d.notes
         from map_solar_systems mss
         left join map_solar_system_details d
             on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
         where mss.id = $2 and mss.map_id = $1",
        map_id,
        map_solar_system_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)?;
    Ok(SystemDetails { notes: row.notes })
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetHome {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub value: bool,
}

/// Mark a placement as the map's home system (or clear it). A map has at most one home (a
/// partial unique index enforces it), so setting a new home first clears the previous one.
pub async fn set_home(pool: &PgPool, actor: Actor, cmd: SetHome) -> Result<()> {
    execute(pool, actor, MapCommand::SetHome(cmd)).await?;
    Ok(())
}

/// The apply half of a one-per-map flag (home, rally): clear whoever holds it, set the new
/// holder, and undo by restoring the previous one. The three statements are passed in as
/// literals so sqlx still checks them against the schema at compile time.
macro_rules! exclusive_flag {
    ($apply:ident, $cmd:ident, $holder:literal, $clear:literal, $set:literal,
     $event:literal, $set_label:literal, $cleared_label:literal) => {
        pub(super) async fn $apply(tx: &mut Tx<'_>, cmd: $cmd) -> Result<Effect> {
            let previous: Option<i64> = sqlx::query_scalar!($holder, cmd.map_id)
                .fetch_optional(&mut **tx)
                .await?;
            if cmd.value {
                sqlx::query!($clear, cmd.map_id).execute(&mut **tx).await?;
            }
            let updated = sqlx::query!($set, cmd.value, cmd.map_solar_system_id, cmd.map_id)
                .execute(&mut **tx)
                .await?
                .rows_affected();
            if updated == 0 {
                return Err(MapError::NotFound);
            }
            let inverse = MapCommand::$cmd(match previous {
                Some(id) => $cmd {
                    map_id: cmd.map_id,
                    map_solar_system_id: id,
                    value: true,
                },
                None => $cmd {
                    map_id: cmd.map_id,
                    map_solar_system_id: cmd.map_solar_system_id,
                    value: false,
                },
            });
            let label = if cmd.value {
                $set_label
            } else {
                $cleared_label
            };
            Ok(Effect::new($event, label, CommandOutput::None).undo_with(inverse))
        }
    };
}

exclusive_flag!(
    apply_set_home,
    SetHome,
    "select id from map_solar_systems where map_id = $1 and is_home",
    "update map_solar_systems set is_home = false where map_id = $1 and is_home",
    "update map_solar_systems set is_home = $1 where id = $2 and map_id = $3",
    "systems.home",
    "set the home system",
    "cleared the home system"
);

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetRally {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub value: bool,
}

/// Mark a placement as the map's rally point (or clear it). One rally per map, enforced by a
/// partial unique index, so setting a new rally first clears the previous one.
pub async fn set_rally(pool: &PgPool, actor: Actor, cmd: SetRally) -> Result<()> {
    execute(pool, actor, MapCommand::SetRally(cmd)).await?;
    Ok(())
}

exclusive_flag!(
    apply_set_rally,
    SetRally,
    "select id from map_solar_systems where map_id = $1 and is_rally",
    "update map_solar_systems set is_rally = false where map_id = $1 and is_rally",
    "update map_solar_systems set is_rally = $1 where id = $2 and map_id = $3",
    "systems.rally",
    "set the rally point",
    "cleared the rally point"
);

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetPinned {
    pub map_id: i64,
    pub map_solar_system_id: i64,
    pub value: bool,
}

/// Pin or unpin a placement. Pinned systems are drag-locked client-side and survive
/// "clear map". Any number of systems may be pinned.
pub async fn set_pinned(pool: &PgPool, actor: Actor, cmd: SetPinned) -> Result<()> {
    execute(pool, actor, MapCommand::SetPinned(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_set_pinned(tx: &mut Tx<'_>, cmd: SetPinned) -> Result<Effect> {
    // Pinning holds the node still, roots the tree layout, and shields it from every sweep,
    // none of which mean anything for a hole nobody has been through.
    if cmd.value {
        let system = sqlx::query_scalar!(
            "select solar_system_id from map_solar_systems where id = $1 and map_id = $2",
            cmd.map_solar_system_id,
            cmd.map_id,
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(MapError::NotFound)?;
        if system.is_none() {
            return Err(MapError::Validation(
                "assign a system to that hole before pinning it".into(),
            ));
        }
    }
    let updated = sqlx::query!(
        "update map_solar_systems set is_pinned = $1 where id = $2 and map_id = $3",
        cmd.value,
        cmd.map_solar_system_id,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(MapError::NotFound);
    }
    let inverse = MapCommand::SetPinned(SetPinned {
        map_id: cmd.map_id,
        map_solar_system_id: cmd.map_solar_system_id,
        value: !cmd.value,
    });
    let label = if cmd.value {
        "pinned a system"
    } else {
        "unpinned a system"
    };
    Ok(Effect::new("systems.pinned", label, CommandOutput::None).undo_with(inverse))
}

// --- restore (the inverse of every removal) ---

/// One removed placement, with everything needed to put it back exactly as it was.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoredSystem {
    pub id: i64,
    pub solar_system_id: Option<i64>,
    pub position_x: f64,
    pub position_y: f64,
    pub alias: Option<String>,
    pub is_home: bool,
    pub is_rally: bool,
    pub is_pinned: bool,
}

/// A connection that cascaded away with its endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoredConnection {
    pub id: i64,
    pub from_system: i64,
    pub to_system: i64,
    pub kind: ConnectionType,
    pub mass_status: Option<MassStatus>,
    pub time_status: Option<TimeStatus>,
    pub size: Option<WormholeSize>,
    pub preserve_mass: bool,
}

/// A signature that cascaded away with its system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoredSignature {
    pub id: i64,
    pub solar_system_id: i64,
    pub signature_id: String,
    pub group: SignatureGroup,
    pub signature_type_id: Option<i64>,
    pub name: Option<String>,
    pub size: Option<WormholeSize>,
    pub mass_status: Option<MassStatus>,
    pub time_status: Option<TimeStatus>,
    pub connection_id: Option<i64>,
}

/// The inverse of a removal: placements plus everything that cascaded with them. Internal,
/// produced by the remove commands and never routed by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreSystems {
    pub map_id: i64,
    pub systems: Vec<RestoredSystem>,
    pub connections: Vec<RestoredConnection>,
    pub signatures: Vec<RestoredSignature>,
}

impl RestoreSystems {
    fn label(&self) -> String {
        match self.systems.len() {
            1 => "a system".to_string(),
            n => format!("{n} systems"),
        }
    }

    /// A connection-only snapshot restores the edge, not any placement.
    fn is_connection_only(&self) -> bool {
        self.systems.is_empty() && !self.connections.is_empty()
    }
}

/// Take the signatures the snapshot claimed that the database will not cascade: the ones in
/// the systems staying on the map, linked to a connection about to die with its endpoint.
/// Must run before the placements go, while their connections still name them.
async fn remove_captured_signatures(
    tx: &mut Tx<'_>,
    map_id: i64,
    snapshot: &RestoreSystems,
) -> Result<()> {
    let ids: Vec<i64> = snapshot.signatures.iter().map(|s| s.id).collect();
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query!(
        "delete from signatures where map_id = $1 and id = any($2)",
        map_id,
        &ids,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Snapshot placements and their cascade so a removal can be undone.
pub(super) async fn capture_systems(
    tx: &mut Tx<'_>,
    map_id: i64,
    ids: &[i64],
) -> Result<RestoreSystems> {
    let systems: Vec<RestoredSystem> = sqlx::query_as!(
        RestoredSystem,
        "select id, solar_system_id, position_x, position_y, alias, is_home, is_rally, is_pinned
         from map_solar_systems where map_id = $1 and id = any($2)",
        map_id,
        ids,
    )
    .fetch_all(&mut **tx)
    .await?;
    // Ghosts hold no signatures, having no system to hold them against.
    let system_ids: Vec<i64> = systems.iter().filter_map(|s| s.solar_system_id).collect();

    let connections = sqlx::query_as!(
        RestoredConnection,
        r#"select id, from_system, to_system, type as "kind: ConnectionType",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  size as "size: WormholeSize", preserve_mass
           from map_connections
           where map_id = $1 and (from_system = any($2) or to_system = any($2))"#,
        map_id,
        ids,
    )
    .fetch_all(&mut **tx)
    .await?;

    // Both sides of the hole: this system's own scan, and the signature in the system
    // across each dying connection.
    let connection_ids: Vec<i64> = connections.iter().map(|c| c.id).collect();
    let signatures = sqlx::query_as!(
        RestoredSignature,
        r#"select id, solar_system_id, signature_id, "group" as "group: SignatureGroup",
                  signature_type_id, name, size as "size: WormholeSize",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus", connection_id
           from signatures
           where map_id = $1
             and (solar_system_id = any($2) or connection_id = any($3))"#,
        map_id,
        &system_ids,
        &connection_ids,
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(RestoreSystems {
        map_id,
        systems,
        connections,
        signatures,
    })
}

pub(super) async fn apply_restore_systems(tx: &mut Tx<'_>, cmd: RestoreSystems) -> Result<Effect> {
    // Original ids are reused so the restored connections and signatures still line up.
    for s in &cmd.systems {
        sqlx::query!(
            "insert into map_solar_systems
                 (id, map_id, solar_system_id, position_x, position_y, alias,
                  is_home, is_rally, is_pinned)
             overriding system value
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             on conflict (id) do nothing",
            s.id,
            cmd.map_id,
            s.solar_system_id,
            s.position_x,
            s.position_y,
            s.alias.as_deref(),
            s.is_home,
            s.is_rally,
            s.is_pinned,
        )
        .execute(&mut **tx)
        .await?;
    }
    for c in &cmd.connections {
        sqlx::query!(
            "insert into map_connections
                 (id, map_id, from_system, to_system, type, mass_status, time_status, size,
                  preserve_mass)
             overriding system value
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             on conflict (id) do nothing",
            c.id,
            cmd.map_id,
            c.from_system,
            c.to_system,
            c.kind.as_str(),
            c.mass_status.map(|m| m.as_str()),
            c.time_status.map(|t| t.as_str()),
            c.size.map(|s| s.as_str()),
            c.preserve_mass,
        )
        .execute(&mut **tx)
        .await?;
    }
    for s in &cmd.signatures {
        sqlx::query!(
            r#"insert into signatures
                   (id, map_id, solar_system_id, signature_id, "group", signature_type_id,
                    name, size, mass_status, time_status, connection_id)
               overriding system value
               values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               on conflict (id) do nothing"#,
            s.id,
            cmd.map_id,
            s.solar_system_id,
            s.signature_id,
            s.group.as_str(),
            s.signature_type_id,
            s.name.as_deref(),
            s.size.map(|w| w.as_str()),
            s.mass_status.map(|m| m.as_str()),
            s.time_status.map(|t| t.as_str()),
            s.connection_id,
        )
        .execute(&mut **tx)
        .await?;
    }

    // Undoing a restore drops exactly what was put back, connections included: a stale-clean
    // can restore edges whose endpoints were never removed, and cascading from the
    // placements alone would leave those behind.
    let inverse = MapCommand::RemoveRestored(RemoveRestored {
        map_id: cmd.map_id,
        system_ids: cmd.systems.iter().map(|s| s.id).collect(),
        connection_ids: cmd.connections.iter().map(|c| c.id).collect(),
    });
    if cmd.is_connection_only() {
        return Ok(Effect::new(
            "connections.restored",
            "restored a connection",
            CommandOutput::None,
        )
        .undo_with(inverse));
    }
    let label = format!("restored {}", cmd.label());
    let count = cmd.systems.len() as i64;
    Ok(Effect::new("systems.restored", label, CommandOutput::None)
        .entries(count)
        .undo_with(inverse))
}

/// The inverse of a restore: drop exactly these placements and edges. The two commands are
/// each other's inverse, so undo and redo cycle without drift.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveRestored {
    pub map_id: i64,
    pub system_ids: Vec<i64>,
    pub connection_ids: Vec<i64>,
}

pub(super) async fn apply_remove_restored(tx: &mut Tx<'_>, cmd: RemoveRestored) -> Result<Effect> {
    let mut snapshot = capture_systems(tx, cmd.map_id, &cmd.system_ids).await?;
    for id in &cmd.connection_ids {
        if let Some(extra) = capture_connection(tx, cmd.map_id, *id).await?
            && !snapshot.connections.iter().any(|c| c.id == *id)
        {
            snapshot.connections.extend(extra.connections);
        }
    }
    sqlx::query!(
        "delete from map_connections where map_id = $1 and id = any($2)",
        cmd.map_id,
        &cmd.connection_ids,
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        "delete from map_solar_systems where map_id = $1 and id = any($2)",
        cmd.map_id,
        &cmd.system_ids,
    )
    .execute(&mut **tx)
    .await?;

    let count = (cmd.system_ids.len() + cmd.connection_ids.len()) as i64;
    Ok(
        Effect::new("systems.removed", "undid a restore", CommandOutput::None)
            .entries(count)
            .undo_with(MapCommand::RestoreSystems(snapshot)),
    )
}

/// Snapshot one connection so removing it can be undone.
pub(super) async fn capture_connection(
    tx: &mut Tx<'_>,
    map_id: i64,
    connection_id: i64,
) -> Result<Option<RestoreSystems>> {
    let connection = sqlx::query_as!(
        RestoredConnection,
        r#"select id, from_system, to_system, type as "kind: ConnectionType",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  size as "size: WormholeSize", preserve_mass
           from map_connections where id = $1 and map_id = $2"#,
        connection_id,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(connection.map(|c| RestoreSystems {
        map_id,
        systems: Vec::new(),
        connections: vec![c],
        signatures: Vec::new(),
    }))
}

/// The current occupier / notes value, erroring when the placement is missing. Two
/// literal queries rather than a built string, so the SQL stays checked at compile time.
async fn detail_text(
    tx: &mut Tx<'_>,
    map_id: i64,
    map_solar_system_id: i64,
    column: DetailColumn,
) -> Result<Option<String>> {
    let value = match column {
        DetailColumn::Occupier => {
            sqlx::query_scalar!(
                "select d.occupying_group
             from map_solar_systems mss
             left join map_solar_system_details d
                 on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
             where mss.id = $1 and mss.map_id = $2",
                map_solar_system_id,
                map_id,
            )
            .fetch_optional(&mut **tx)
            .await?
        }
        DetailColumn::Notes => {
            sqlx::query_scalar!(
                "select d.notes
             from map_solar_systems mss
             left join map_solar_system_details d
                 on d.map_id = mss.map_id and d.solar_system_id = mss.solar_system_id
             where mss.id = $1 and mss.map_id = $2",
                map_solar_system_id,
                map_id,
            )
            .fetch_optional(&mut **tx)
            .await?
        }
    };
    value.ok_or(MapError::NotFound)
}

enum DetailColumn {
    Occupier,
    Notes,
}

/// A command returned an output shape its wrapper doesn't expect: a bug, not a user error.
pub(super) fn unexpected(output: CommandOutput) -> MapError {
    MapError::Validation(format!("unexpected command output: {output:?}"))
}
