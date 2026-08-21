//! Cosmic signatures scanned in a system, and linking a wormhole signature to a
//! connection. All mutations are Member+; reads are Viewer+ (see
//! [access.md](../../docs/database/access.md)).
//!
//! A signature may reference a catalog type (`signature_types`); `name` holds the raw
//! scanner type name when nothing matched. Only `wormhole`-group signatures carry
//! `mass_status` / `time_status` / `size` or a connection. Linking sets `connection_id`,
//! which fires the `map_*_sync` triggers (migration 0009): the connection and its
//! signatures reconcile to the worst state per field, then stay in lock-step.
//!
//! Paste ([`paste_signatures`]) is upsert-only: it never deletes. Rows that vanished from a
//! scan go through [`remove_signatures`], which cascades: a connection dies with its last
//! same-side signature, and endpoints left unpinned, unmarked and connection-less leave the
//! map.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{MassStatus, SignatureGroup, TimeStatus, WormholeSize};

use sqlx::PgPool;

use super::access::require_role;
use super::command::{CommandOutput, Effect, MapCommand, Tx, execute};
use super::error::{MapError, Result};
use super::{Actor, MapEvent, MapHub, Role};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Signature {
    pub id: i64,
    pub map_id: i64,
    pub solar_system_id: i64,
    /// The in-game scanner id, e.g. `ABC-123`. Unique per `(map, system)`.
    pub signature_id: String,
    pub group: SignatureGroup,
    /// The matched catalog type ([`signature_types`](../../docs/database/static.md)).
    pub signature_type_id: Option<i64>,
    /// The raw scanner type name when no catalog type matched.
    pub name: Option<String>,
    pub size: Option<WormholeSize>,
    pub mass_status: Option<MassStatus>,
    pub time_status: Option<TimeStatus>,
    /// When `time_status` last changed (DB trigger), for "EOL since" displays.
    pub time_status_updated_at: Option<DateTime<Utc>>,
    /// The connection this signature is one end of, if linked. Only `wormhole` sigs.
    pub connection_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Only this group may carry life-cycle state (`size`/`mass`/`time`) and a connection link.
fn is_wormhole(group: SignatureGroup) -> bool {
    group == SignatureGroup::Wormhole
}

/// The `signature_categories` id a group maps to; `Unknown` has no catalog category.
fn category_id_for(group: SignatureGroup) -> Option<i64> {
    match group {
        SignatureGroup::Wormhole => Some(1),
        SignatureGroup::Data => Some(2),
        SignatureGroup::Relic => Some(3),
        SignatureGroup::Combat => Some(4),
        SignatureGroup::Gas => Some(5),
        SignatureGroup::Ore => Some(6),
        SignatureGroup::Homefront => Some(7),
        SignatureGroup::Unknown => None,
    }
}

/// Scanner ids are exactly 7 chars (`ABC-123`).
fn validate_signature_id(id: &str) -> Result<()> {
    if id.len() != 7 {
        return Err(MapError::Validation(
            "signature id must be 7 characters (ABC-123)".into(),
        ));
    }
    Ok(())
}

/// `Validation` unless the catalog type exists and belongs to the group's category.
async fn validate_type_for_group(
    tx: &mut Tx<'_>,
    type_id: i64,
    group: SignatureGroup,
) -> Result<()> {
    let category = sqlx::query_scalar!(
        "select signature_category_id from signature_types where id = $1",
        type_id,
    )
    .fetch_optional(&mut **tx)
    .await?;
    if category.is_none() || category != category_id_for(group) {
        return Err(MapError::Validation(
            "signature type does not belong to the signature's category".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AddSignature {
    pub map_id: i64,
    pub solar_system_id: i64,
    pub signature_id: String,
    pub group: SignatureGroup,
    #[serde(default)]
    #[ts(optional)]
    pub signature_type_id: Option<i64>,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub size: Option<WormholeSize>,
    #[serde(default)]
    #[ts(optional)]
    pub mass_status: Option<MassStatus>,
    #[serde(default)]
    #[ts(optional)]
    pub time_status: Option<TimeStatus>,
}

impl AddSignature {
    pub fn validate(&self) -> Result<()> {
        validate_signature_id(self.signature_id.trim())?;
        // Wormhole state only belongs on a wormhole signature.
        if !is_wormhole(self.group)
            && (self.size.is_some() || self.mass_status.is_some() || self.time_status.is_some())
        {
            return Err(MapError::Validation(
                "only a wormhole signature can carry size / mass / time".into(),
            ));
        }
        Ok(())
    }
}

/// Record a scanned signature in a placed system. The system must be on this map. A
/// duplicate `signature_id` in the same system → `Conflict`. Linking to a connection is a
/// separate step ([`link_signature`]).
pub async fn add_signature(pool: &PgPool, actor: Actor, cmd: AddSignature) -> Result<Signature> {
    execute(pool, actor, MapCommand::AddSignature(cmd))
        .await?
        .signature()
}

pub(super) async fn apply_add_signature(tx: &mut Tx<'_>, cmd: AddSignature) -> Result<Effect> {
    cmd.validate()?;
    ensure_system_placed(tx, cmd.map_id, cmd.solar_system_id).await?;
    if let Some(type_id) = cmd.signature_type_id {
        validate_type_for_group(tx, type_id, cmd.group).await?;
    }

    let exists = sqlx::query_scalar!(
        "select exists(
             select 1 from signatures
             where map_id = $1 and solar_system_id = $2 and signature_id = $3
         )",
        cmd.map_id,
        cmd.solar_system_id,
        cmd.signature_id.trim(),
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(false);
    if exists {
        return Err(MapError::Conflict("signature already scanned here".into()));
    }

    // The stamp trigger only fires on update, so an initial time_status stamps here.
    let sig = sqlx::query_as!(
        Signature,
        r#"insert into signatures
               (map_id, solar_system_id, signature_id, "group", signature_type_id, name,
                size, mass_status, time_status, time_status_updated_at)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9,
                   -- $9 is the lifetime, whose type the parameter already fixes; naming it
                   -- again as text is what made the two deductions disagree.
                   case when $9::time_status is not null then now() end)
           returning id, map_id, solar_system_id, signature_id, "group",
                     signature_type_id, name, size,
                     mass_status,
                     time_status,
                     time_status_updated_at, connection_id, created_at, updated_at"#,
        cmd.map_id,
        cmd.solar_system_id,
        cmd.signature_id.trim(),
        cmd.group,
        cmd.signature_type_id,
        cmd.name.as_deref(),
        cmd.size,
        cmd.mass_status,
        cmd.time_status,
    )
    .fetch_one(&mut **tx)
    .await?;
    let inverse = MapCommand::RemoveSignature(RemoveSignature {
        map_id: cmd.map_id,
        signature_pk: sig.id,
    });
    Ok(Effect::new(
        "signatures.added",
        format!("added signature {}", sig.signature_id),
        CommandOutput::Signature(Box::new(sig)),
    )
    .undo_with(inverse))
}

/// A partial edit of a signature. `None` leaves a field unchanged; `Some(None)` clears it.
/// Changing the group clears the catalog type, the connection link, and any wormhole state
/// (matching the legacy category select), unless a new type is supplied in the same call.
/// Editing a linked wormhole's state propagates to its connection via the DB trigger.
#[derive(Debug, Default, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateSignature {
    pub map_id: i64,
    pub signature_pk: i64,
    /// `Some` renames the scanner id (7 chars; duplicate in the system → `Conflict`).
    #[serde(default)]
    #[ts(optional)]
    pub signature_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub group: Option<SignatureGroup>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub signature_type_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub size: Option<Option<WormholeSize>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub mass_status: Option<Option<MassStatus>>,
    #[serde(default, deserialize_with = "super::double_option")]
    #[ts(optional)]
    pub time_status: Option<Option<TimeStatus>>,
}

pub async fn update_signature(
    pool: &PgPool,
    actor: Actor,
    cmd: UpdateSignature,
) -> Result<Signature> {
    execute(pool, actor, MapCommand::UpdateSignature(cmd))
        .await?
        .signature()
}

pub(super) async fn apply_update_signature(
    tx: &mut Tx<'_>,
    cmd: UpdateSignature,
) -> Result<Effect> {
    let current = fetch_signature_tx(tx, cmd.map_id, cmd.signature_pk).await?;
    let previous_name = current.name.clone();

    let signature_id = match &cmd.signature_id {
        Some(v) => {
            let v = v.trim();
            validate_signature_id(v)?;
            if v != current.signature_id {
                let taken = sqlx::query_scalar!(
                    "select exists(
                         select 1 from signatures
                         where map_id = $1 and solar_system_id = $2 and signature_id = $3
                     )",
                    cmd.map_id,
                    current.solar_system_id,
                    v,
                )
                .fetch_one(&mut **tx)
                .await?
                .unwrap_or(false);
                if taken {
                    return Err(MapError::Conflict("signature already scanned here".into()));
                }
            }
            v.to_string()
        }
        None => current.signature_id.clone(),
    };

    let group = cmd.group.unwrap_or(current.group);
    let group_changed = group != current.group;

    let type_id = if group_changed {
        cmd.signature_type_id.flatten()
    } else {
        cmd.signature_type_id.unwrap_or(current.signature_type_id)
    };
    if let Some(type_id) = type_id {
        validate_type_for_group(tx, type_id, group).await?;
    }

    let name = cmd.name.unwrap_or(current.name);
    let (size, mass, time) = if group_changed && !is_wormhole(group) {
        (None, None, None)
    } else {
        (
            cmd.size.unwrap_or(current.size),
            cmd.mass_status.unwrap_or(current.mass_status),
            cmd.time_status.unwrap_or(current.time_status),
        )
    };
    let connection_id = if group_changed {
        None
    } else {
        current.connection_id
    };

    if !is_wormhole(group) && (size.is_some() || mass.is_some() || time.is_some()) {
        return Err(MapError::Validation(
            "only a wormhole signature can carry size / mass / time".into(),
        ));
    }

    let sig = sqlx::query_as!(
        Signature,
        r#"update signatures
           set signature_id = $1, "group" = $2, signature_type_id = $3, name = $4, size = $5,
               mass_status = $6, time_status = $7, connection_id = $8, updated_at = now()
           where id = $9 and map_id = $10
           returning id, map_id, solar_system_id, signature_id, "group",
                     signature_type_id, name, size,
                     mass_status,
                     time_status,
                     time_status_updated_at, connection_id, created_at, updated_at"#,
        signature_id,
        group,
        type_id,
        name.as_deref(),
        size,
        mass,
        time,
        connection_id,
        cmd.signature_pk,
        cmd.map_id,
    )
    .fetch_one(&mut **tx)
    .await?;
    let inverse = MapCommand::UpdateSignature(UpdateSignature {
        map_id: cmd.map_id,
        signature_pk: cmd.signature_pk,
        signature_id: Some(current.signature_id.clone()),
        group: Some(current.group),
        signature_type_id: Some(current.signature_type_id),
        name: Some(previous_name),
        size: Some(current.size),
        mass_status: Some(current.mass_status),
        time_status: Some(current.time_status),
    });
    Ok(Effect::new(
        "signatures.updated",
        format!("edited signature {}", sig.signature_id),
        CommandOutput::Signature(Box::new(sig)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveSignature {
    pub map_id: i64,
    pub signature_pk: i64,
}

/// What a delete touched, so the caller can publish the right events.
#[derive(Debug, Clone)]
pub struct RemovedSignature {
    pub solar_system_id: i64,
    pub removed_connection_id: Option<i64>,
    /// Ghosts the removed connection stranded. A real system is left where it is.
    pub removed_placement_ids: Vec<i64>,
}

/// Delete a signature. Legacy cascade: if it was the last signature on its side of a
/// linked connection, the connection goes too (the other end's signature, if any, is
/// unlinked by the FK). Real endpoint systems are left in place; that cleanup belongs to
/// the bulk path ([`remove_signatures`]). A ghost is not one of those: it exists only as
/// the far side of the hole this signature described, so it goes with the connection.
pub async fn remove_signature(
    pool: &PgPool,
    actor: Actor,
    cmd: RemoveSignature,
) -> Result<RemovedSignature> {
    execute(pool, actor, MapCommand::RemoveSignature(cmd))
        .await?
        .removal()
}

pub(super) async fn apply_remove_signature(
    tx: &mut Tx<'_>,
    cmd: RemoveSignature,
) -> Result<Effect> {
    let restore = capture_signatures(tx, cmd.map_id, &[cmd.signature_pk]).await?;
    // The node this scan raised goes with it, by the foreign key. Snapshotted first, so
    // one undo brings the scan and what it drew back together.
    let raised = super::ghost::raised_by(tx, cmd.map_id, &[cmd.signature_pk]).await?;
    let raised_snapshot = capture_raised(tx, cmd.map_id, &raised).await?;

    let sig = sqlx::query!(
        "delete from signatures where id = $1 and map_id = $2
         returning solar_system_id, connection_id",
        cmd.signature_pk,
        cmd.map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)?;

    let mut removed_connection_id = None;
    if let Some(conn_id) = sig.connection_id
        && delete_connection_if_side_empty(tx, cmd.map_id, conn_id, sig.solar_system_id)
            .await?
            .is_some()
    {
        removed_connection_id = Some(conn_id);
    }

    Ok(Effect::new(
        "signatures.removed",
        "removed a signature",
        CommandOutput::Removal(Box::new(RemovedSignature {
            solar_system_id: sig.solar_system_id,
            removed_connection_id,
            removed_placement_ids: raised,
        })),
    )
    .undo_with(undo_with_raised(
        cmd.map_id,
        raised_snapshot,
        MapCommand::RestoreSignatures(restore),
    )))
}

async fn capture_raised(
    tx: &mut Tx<'_>,
    map_id: i64,
    raised: &[i64],
) -> Result<Option<super::solar_system::RestoreSystems>> {
    if raised.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        super::solar_system::capture_systems(tx, map_id, raised).await?,
    ))
}

/// Undo of a delete that took nodes with it. The nodes go back first: their snapshot
/// carries the edge, and the scan cannot name an edge that is not there yet. It carries
/// the scan too, so the second step is usually the one that finds nothing left to do.
fn undo_with_raised(
    map_id: i64,
    raised: Option<super::solar_system::RestoreSystems>,
    inverse: MapCommand,
) -> MapCommand {
    let Some(raised) = raised else { return inverse };
    MapCommand::Sequence(super::command::Sequence {
        map_id,
        steps: vec![MapCommand::RestoreSystems(raised), inverse],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveSignatures {
    pub map_id: i64,
    pub signature_pks: Vec<i64>,
}

/// What the bulk delete touched, so the caller can publish the right events.
#[derive(Debug, Clone)]
pub struct BulkRemoveOutcome {
    /// Solar systems that lost signatures.
    pub systems: Vec<i64>,
    pub removed_connection_ids: Vec<i64>,
    /// Placements (`map_solar_systems.id`) removed by the orphan cleanup.
    pub removed_placement_ids: Vec<i64>,
}

/// Bulk delete (the panel's "delete missing signatures" path), with the full legacy
/// cascade: each linked connection whose side is left without signatures is deleted, and
/// endpoint placements that are not pinned / home / rally and have no remaining
/// connections are removed from the map (taking their own signatures with them).
pub async fn remove_signatures(
    pool: &PgPool,
    actor: Actor,
    cmd: RemoveSignatures,
) -> Result<BulkRemoveOutcome> {
    execute(pool, actor, MapCommand::RemoveSignatures(cmd))
        .await?
        .bulk_removal()
}

pub(super) async fn apply_remove_signatures(
    tx: &mut Tx<'_>,
    cmd: RemoveSignatures,
) -> Result<Effect> {
    let restore = capture_signatures(tx, cmd.map_id, &cmd.signature_pks).await?;
    let raised = super::ghost::raised_by(tx, cmd.map_id, &cmd.signature_pks).await?;
    let raised_snapshot = capture_raised(tx, cmd.map_id, &raised).await?;
    let removed = sqlx::query!(
        "delete from signatures where map_id = $1 and id = any($2)
         returning solar_system_id, connection_id",
        cmd.map_id,
        &cmd.signature_pks,
    )
    .fetch_all(&mut **tx)
    .await?;

    let mut systems: Vec<i64> = removed.iter().map(|r| r.solar_system_id).collect();
    systems.sort_unstable();
    systems.dedup();

    // Each (connection, side) pair at most once, then the legacy side-empty rule.
    let mut sides: Vec<(i64, i64)> = removed
        .iter()
        .filter_map(|r| r.connection_id.map(|c| (c, r.solar_system_id)))
        .collect();
    sides.sort_unstable();
    sides.dedup();

    let mut removed_connection_ids = Vec::new();
    let mut endpoint_candidates: Vec<i64> = Vec::new();
    for (conn_id, side_system) in sides {
        if let Some(endpoints) =
            delete_connection_if_side_empty(tx, cmd.map_id, conn_id, side_system).await?
        {
            removed_connection_ids.push(conn_id);
            endpoint_candidates.extend(endpoints);
        }
    }
    endpoint_candidates.sort_unstable();
    endpoint_candidates.dedup();

    // Orphan cleanup: endpoints of the deleted connections that are unpinned, unmarked,
    // and now connection-less disappear from the map.
    let mut removed_placement_ids = raised;
    for placement_id in endpoint_candidates {
        let deleted = sqlx::query!(
            "delete from map_solar_systems mss
             where mss.id = $1 and mss.map_id = $2
               and not mss.is_pinned and not mss.is_home and not mss.is_rally
               and not exists (
                   select 1 from map_connections c
                   where c.map_id = $2 and (c.from_system = $1 or c.to_system = $1)
               )",
            placement_id,
            cmd.map_id,
        )
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if deleted > 0 {
            removed_placement_ids.push(placement_id);
        }
    }

    let count = cmd.signature_pks.len() as i64;
    Ok(Effect::new(
        "signatures.removed",
        format!("removed {count} signatures"),
        CommandOutput::BulkRemoval(Box::new(BulkRemoveOutcome {
            systems,
            removed_connection_ids,
            removed_placement_ids,
        })),
    )
    .entries(count)
    .undo_with(undo_with_raised(
        cmd.map_id,
        raised_snapshot,
        MapCommand::RestoreSignatures(restore),
    )))
}

/// Delete `conn_id` if no signature in `side_system` still references it (the legacy
/// same-side survivor rule). Returns the connection's endpoint placement ids when it was
/// deleted, `None` when a survivor kept it alive.
async fn delete_connection_if_side_empty(
    tx: &mut Tx<'_>,
    map_id: i64,
    conn_id: i64,
    side_system: i64,
) -> Result<Option<[i64; 2]>> {
    let survivors = sqlx::query_scalar!(
        "select count(*) from signatures
         where connection_id = $1 and solar_system_id = $2",
        conn_id,
        side_system,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(0);
    if survivors > 0 {
        return Ok(None);
    }
    let endpoints = sqlx::query!(
        "delete from map_connections where id = $1 and map_id = $2
         returning from_system, to_system",
        conn_id,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(endpoints.map(|e| [e.from_system, e.to_system]))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct LinkSignature {
    pub map_id: i64,
    pub signature_pk: i64,
    pub connection_id: i64,
}

/// Link a wormhole signature to a connection as one of its ends. Fires the DB sync, which
/// reconciles the connection and its signatures to the worst state per field (then keeps
/// them in lock-step). Returns the signature *after* the merge. The signature must be a
/// `wormhole`, and the connection must be on this map and have an endpoint in the
/// signature's system.
pub async fn link_signature(pool: &PgPool, actor: Actor, cmd: LinkSignature) -> Result<Signature> {
    execute(pool, actor, MapCommand::LinkSignature(cmd))
        .await?
        .signature()
}

pub(super) async fn apply_link_signature(tx: &mut Tx<'_>, cmd: LinkSignature) -> Result<Effect> {
    let sig = fetch_signature_tx(tx, cmd.map_id, cmd.signature_pk).await?;
    if !is_wormhole(sig.group) {
        return Err(MapError::Validation(
            "only a wormhole signature can link to a connection".into(),
        ));
    }

    // The connection must be on this map and touch the signature's system (one of its two
    // endpoint placements is in that solar system).
    let ok = sqlx::query_scalar!(
        "select exists(
             select 1
             from map_connections c
             join map_solar_systems mss on mss.id in (c.from_system, c.to_system)
             where c.id = $1 and c.map_id = $2 and mss.solar_system_id = $3
         )",
        cmd.connection_id,
        cmd.map_id,
        sig.solar_system_id,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(false);
    if !ok {
        return Err(MapError::Validation(
            "connection must be on this map and reach the signature's system".into(),
        ));
    }

    sqlx::query!(
        "update signatures set connection_id = $1 where id = $2 and map_id = $3",
        cmd.connection_id,
        cmd.signature_pk,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;

    // Re-read: the merge trigger may have changed this row's state after the UPDATE.
    let linked = fetch_signature_tx(tx, cmd.map_id, cmd.signature_pk).await?;
    // Undo unlinks and restores the pre-merge life-cycle state.
    let inverse = MapCommand::UpdateSignature(UpdateSignature {
        map_id: cmd.map_id,
        signature_pk: cmd.signature_pk,
        signature_id: None,
        group: None,
        signature_type_id: None,
        name: None,
        size: Some(sig.size),
        mass_status: Some(sig.mass_status),
        time_status: Some(sig.time_status),
    });
    Ok(Effect::new(
        "signatures.linked",
        "linked a signature",
        CommandOutput::Signature(Box::new(linked)),
    )
    .undo_with(inverse))
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UnlinkSignature {
    pub map_id: i64,
    pub signature_pk: i64,
}

/// Detach a signature from its connection. The signature keeps its last state as a standalone
/// scanned wormhole; the connection and any sibling signature are untouched.
pub async fn unlink_signature(
    pool: &PgPool,
    actor: Actor,
    cmd: UnlinkSignature,
) -> Result<Signature> {
    execute(pool, actor, MapCommand::UnlinkSignature(cmd))
        .await?
        .signature()
}

pub(super) async fn apply_unlink_signature(
    tx: &mut Tx<'_>,
    cmd: UnlinkSignature,
) -> Result<Effect> {
    let before = fetch_signature_tx(tx, cmd.map_id, cmd.signature_pk).await?;
    sqlx::query!(
        "update signatures set connection_id = null where id = $1 and map_id = $2",
        cmd.signature_pk,
        cmd.map_id,
    )
    .execute(&mut **tx)
    .await?;
    let sig = fetch_signature_tx(tx, cmd.map_id, cmd.signature_pk).await?;
    // Unlinking something that was never linked has nothing to restore.
    let inverse = before.connection_id.map(|connection_id| {
        MapCommand::LinkSignature(LinkSignature {
            map_id: cmd.map_id,
            signature_pk: cmd.signature_pk,
            connection_id,
        })
    });
    let mut effect = Effect::new(
        "signatures.unlinked",
        "unlinked a signature",
        CommandOutput::Signature(Box::new(sig)),
    );
    if let Some(inverse) = inverse {
        effect = effect.undo_with(inverse);
    }
    Ok(effect)
}

/// One row of a parsed scanner paste. `group` is `None` when the scanner line carried no
/// classification (legacy: keep whatever the row already has). The catalog type is only
/// ever client-matched for site categories; wormhole types never come from a paste.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PastedSignature {
    pub signature_id: String,
    #[serde(default)]
    #[ts(optional)]
    pub group: Option<SignatureGroup>,
    #[serde(default)]
    #[ts(optional)]
    pub signature_type_id: Option<i64>,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PasteSignatures {
    pub map_id: i64,
    pub solar_system_id: i64,
    pub signatures: Vec<PastedSignature>,
}

/// Upsert a pasted in-game scan into a system's signatures. Legacy semantics, so this never
/// deletes anything (the panel diffs client-side and calls [`remove_signatures`]):
///
/// - group: pasted value, else keep
/// - catalog type: an existing wormhole type always survives; a site row takes the
///   pasted match, or is cleared when the paste carries only an unmatched raw name
/// - connection link: kept unless the paste recategorizes the row to a non-wormhole group
/// - raw name: refreshed for sites, preserved for wormholes
/// - `size` / `mass_status` / `time_status` / `created_at`: never touched
pub async fn paste_signatures(pool: &PgPool, actor: Actor, cmd: PasteSignatures) -> Result<()> {
    execute(pool, actor, MapCommand::PasteSignatures(cmd)).await?;
    Ok(())
}

pub(super) async fn apply_paste_signatures(
    tx: &mut Tx<'_>,
    cmd: PasteSignatures,
) -> Result<Effect> {
    ensure_system_placed(tx, cmd.map_id, cmd.solar_system_id).await?;
    // Snapshot the whole system: a paste both edits existing rows and adds new ones.
    let existing_ids: Vec<i64> = sqlx::query_scalar!(
        "select id from signatures where map_id = $1 and solar_system_id = $2",
        cmd.map_id,
        cmd.solar_system_id,
    )
    .fetch_all(&mut **tx)
    .await?;
    let mut restore = capture_signatures(tx, cmd.map_id, &existing_ids).await?;
    restore.replaces_system = Some(cmd.solar_system_id);

    for s in &cmd.signatures {
        let sid = s.signature_id.trim();
        if sid.is_empty() {
            continue;
        }
        validate_signature_id(sid)?;

        // A pasted type only counts if it belongs to the pasted group's category.
        let mut pasted_type = None;
        if let (Some(type_id), Some(group)) = (s.signature_type_id, s.group)
            && type_category(tx, type_id).await? == category_id_for(group)
        {
            pasted_type = Some(type_id);
        }

        let existing = sqlx::query!(
            r#"select id, "group", signature_type_id, name
               from signatures
               where map_id = $1 and solar_system_id = $2 and signature_id = $3
               for update"#,
            cmd.map_id,
            cmd.solar_system_id,
            sid,
        )
        .fetch_optional(&mut **tx)
        .await?;

        let Some(existing) = existing else {
            sqlx::query!(
                r#"insert into signatures
                       (map_id, solar_system_id, signature_id, "group", signature_type_id, name)
                   values ($1, $2, $3, $4, $5, $6)"#,
                cmd.map_id,
                cmd.solar_system_id,
                sid,
                s.group.unwrap_or_default(),
                pasted_type,
                s.name.as_deref(),
            )
            .execute(&mut **tx)
            .await?;
            continue;
        };

        let group = s.group.unwrap_or(existing.group);
        let type_id = if is_wormhole(group) {
            existing.signature_type_id.or(pasted_type)
        } else if pasted_type.is_some() {
            pasted_type
        } else if s.name.is_some() {
            // A site with only an unmatched raw name clears a stale type.
            None
        } else if let Some(kept) = existing.signature_type_id {
            // Keep the old type only if it still fits the (possibly new) group.
            (type_category(tx, kept).await? == category_id_for(group)).then_some(kept)
        } else {
            None
        };
        let name = if is_wormhole(group) {
            existing.name.or_else(|| s.name.clone())
        } else {
            s.name.clone().or(existing.name)
        };
        // Recategorizing a hole into a site drops the link (legacy rule); the connection
        // itself stays on the map.
        let clear_link = s.group.is_some_and(|g| !is_wormhole(g));

        sqlx::query!(
            r#"update signatures
               set "group" = $1, signature_type_id = $2, name = $3,
                   connection_id = case when $4 then null else connection_id end,
                   updated_at = now()
               where id = $5"#,
            group,
            type_id,
            name.as_deref(),
            clear_link,
            existing.id,
        )
        .execute(&mut **tx)
        .await?;
    }

    let count = cmd.signatures.len() as i64;
    Ok(Effect::new(
        "signatures.pasted",
        format!("pasted {count} signatures"),
        CommandOutput::None,
    )
    .entries(count)
    .undo_with(MapCommand::RestoreSignatures(restore)))
}

/// A catalog type's category id, or `None` for an unknown type id.
async fn type_category(tx: &mut Tx<'_>, type_id: i64) -> Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "select signature_category_id from signature_types where id = $1",
        type_id,
    )
    .fetch_optional(&mut **tx)
    .await?)
}

/// Every signature on a map. Viewer+.
pub async fn list_signatures(pool: &PgPool, actor: Actor, map_id: i64) -> Result<Vec<Signature>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    read_signatures(pool, map_id).await
}

/// The scan, for a caller whose right to read the map has already been settled.
pub async fn read_signatures(pool: &PgPool, map_id: i64) -> Result<Vec<Signature>> {
    let sigs = sqlx::query_as!(
        Signature,
        r#"select id, map_id, solar_system_id, signature_id, "group",
                  signature_type_id, name, size,
                  mass_status,
                  time_status,
                  time_status_updated_at, connection_id, created_at, updated_at
           from signatures where map_id = $1 order by solar_system_id, signature_id"#,
        map_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(sigs)
}

/// A snapshot of signatures, the inverse of any signature removal or paste.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreSignatures {
    pub map_id: i64,
    pub signatures: Vec<super::solar_system::RestoredSignature>,
    /// When set, the system is reset to exactly this snapshot (used by paste undo, which
    /// must also drop rows the paste created).
    pub replaces_system: Option<i64>,
}

/// Snapshot signature rows so a removal or paste can be undone.
pub(super) async fn capture_signatures(
    tx: &mut Tx<'_>,
    map_id: i64,
    ids: &[i64],
) -> Result<RestoreSignatures> {
    let signatures = sqlx::query_as!(
        super::solar_system::RestoredSignature,
        r#"select id, solar_system_id, signature_id, "group",
                  signature_type_id, name, size,
                  mass_status,
                  time_status, connection_id
           from signatures where map_id = $1 and id = any($2)"#,
        map_id,
        ids,
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(RestoreSignatures {
        map_id,
        signatures,
        replaces_system: None,
    })
}

pub(super) async fn apply_restore_signatures(
    tx: &mut Tx<'_>,
    cmd: RestoreSignatures,
) -> Result<Effect> {
    // Paste undo: clear whatever the paste left behind before replaying the snapshot.
    if let Some(solar_system_id) = cmd.replaces_system {
        sqlx::query!(
            "delete from signatures where map_id = $1 and solar_system_id = $2",
            cmd.map_id,
            solar_system_id,
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
               on conflict (id) do update set
                   "group" = excluded."group",
                   signature_type_id = excluded.signature_type_id,
                   name = excluded.name,
                   size = excluded.size,
                   mass_status = excluded.mass_status,
                   time_status = excluded.time_status,
                   connection_id = excluded.connection_id"#,
            s.id,
            cmd.map_id,
            s.solar_system_id,
            s.signature_id,
            s.group,
            s.signature_type_id,
            s.name.as_deref(),
            s.size,
            s.mass_status,
            s.time_status,
            s.connection_id,
        )
        .execute(&mut **tx)
        .await?;
    }
    let count = cmd.signatures.len() as i64;
    let ids: Vec<i64> = cmd.signatures.iter().map(|s| s.id).collect();
    Ok(Effect::new(
        "signatures.restored",
        format!("restored {count} signatures"),
        CommandOutput::None,
    )
    .entries(count)
    .undo_with(MapCommand::RemoveSignatures(RemoveSignatures {
        map_id: cmd.map_id,
        signature_pks: ids,
    })))
}

async fn fetch_signature_tx(tx: &mut Tx<'_>, map_id: i64, signature_pk: i64) -> Result<Signature> {
    sqlx::query_as!(
        Signature,
        r#"select id, map_id, solar_system_id, signature_id, "group",
                  signature_type_id, name, size,
                  mass_status,
                  time_status,
                  time_status_updated_at, connection_id, created_at, updated_at
           from signatures where id = $1 and map_id = $2"#,
        signature_pk,
        map_id,
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(MapError::NotFound)
}

/// `Validation` if the solar system isn't currently placed on the map (signatures hang off
/// a placement via the `(map_id, solar_system_id)` foreign key).
async fn ensure_system_placed(tx: &mut Tx<'_>, map_id: i64, solar_system_id: i64) -> Result<()> {
    let placed = sqlx::query_scalar!(
        "select exists(select 1 from map_solar_systems where map_id = $1 and solar_system_id = $2)",
        map_id,
        solar_system_id,
    )
    .fetch_one(&mut **tx)
    .await?
    .unwrap_or(false);
    if !placed {
        return Err(MapError::Validation(
            "the signature's system is not placed on this map".into(),
        ));
    }
    Ok(())
}

/// Purge stale signatures (legacy expiry): unlinked wormhole sigs older than 3 days, other
/// sigs untouched for 7 days (presence in a paste refreshes `updated_at`, keeping live
/// sites alive). Linked sigs are never expired: they represent a mapped connection.
/// Returns the affected `(map_id, solar_system_id)` pairs.
pub async fn expire_signatures(pool: &PgPool) -> Result<Vec<(i64, i64)>> {
    let rows = sqlx::query!(
        r#"delete from signatures
           where connection_id is null
             and (("group" = 'wormhole' and created_at < now() - interval '3 days')
                  or ("group" <> 'wormhole' and updated_at < now() - interval '7 days'))
           returning map_id, solar_system_id"#,
    )
    .fetch_all(pool)
    .await?;
    let mut pairs: Vec<(i64, i64)> = rows
        .into_iter()
        .map(|r| (r.map_id, r.solar_system_id))
        .collect();
    pairs.sort_unstable();
    pairs.dedup();
    Ok(pairs)
}

/// Publishes a `SignatureChanged` per affected system so open maps refresh. The cutoffs are
/// days, so the tick cadence is uncritical.
pub fn start_expiry(pool: PgPool, hub: MapHub) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(6 * 60 * 60));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match expire_signatures(&pool).await {
                Ok(pairs) => {
                    for (map_id, solar_system_id) in pairs {
                        hub.publish(MapEvent::SignatureChanged {
                            map_id,
                            solar_system_id,
                        });
                    }
                }
                Err(err) => eprintln!("signature expiry failed: {err}"),
            }
        }
    });
}
