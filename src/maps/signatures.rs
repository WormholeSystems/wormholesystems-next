//! Cosmic signatures scanned in a system, and linking a wormhole signature to a
//! connection. All mutations are Member+; reads are Viewer+ (see
//! [access.md](../../docs/database/access.md)).
//!
//! A wormhole signature carries the hole's `mass_status` / `time_status` / `size` straight
//! from the scanner — before it's ever linked to a connection. Linking (`link_signature`)
//! sets `connection_id`, which fires the `map_*_sync` DB triggers (migration 0009): the
//! connection and its signatures reconcile to the worst state per field, then stay in
//! lock-step. Only `wormhole`-group signatures may carry a connection or wormhole state.
//!
//! Scanner-paste reconciliation (bulk add/remove from a pasted scan) is a separate, future
//! action; this module is the structured per-signature CRUD + linking it builds on.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Used by the cross-target struct + command definitions.
use super::{MassStatus, SignatureGroup, TimeStatus, WormholeSize};

use sqlx::PgPool;

use super::access::require_role;
use super::error::{MapError, Result};
use super::{Actor, Role};

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct Signature {
    pub id: i64,
    pub map_id: i64,
    pub solar_system_id: i64,
    /// The in-game scanner id, e.g. `ABC-123`. Unique per `(map, system)`.
    pub signature_id: String,
    pub group: SignatureGroup,
    pub name: Option<String>,
    pub size: Option<WormholeSize>,
    pub mass_status: Option<MassStatus>,
    pub time_status: Option<TimeStatus>,
    /// The connection this signature is one end of, if linked. Only `wormhole` sigs.
    pub connection_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Whether a group is allowed to carry wormhole life-cycle state (`size`/`mass`/`time`) and
/// a connection link.
fn is_wormhole(group: SignatureGroup) -> bool {
    group == SignatureGroup::Wormhole
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
        if self.signature_id.trim().is_empty() {
            return Err(MapError::Validation(
                "signature id must not be blank".into(),
            ));
        }
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
    cmd.validate()?;
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    ensure_system_placed(pool, cmd.map_id, cmd.solar_system_id).await?;

    let exists = sqlx::query_scalar!(
        "select exists(
             select 1 from signatures
             where map_id = $1 and solar_system_id = $2 and signature_id = $3
         )",
        cmd.map_id,
        cmd.solar_system_id,
        cmd.signature_id.trim(),
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if exists {
        return Err(MapError::Conflict("signature already scanned here".into()));
    }

    let sig = sqlx::query_as!(
        Signature,
        r#"insert into signatures
               (map_id, solar_system_id, signature_id, "group", name, size, mass_status, time_status)
           values ($1, $2, $3, $4, $5, $6, $7, $8)
           returning id, map_id, solar_system_id, signature_id, "group" as "group: SignatureGroup",
                     name, size as "size: WormholeSize",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     connection_id, created_at, updated_at"#,
        cmd.map_id,
        cmd.solar_system_id,
        cmd.signature_id.trim(),
        cmd.group.as_str(),
        cmd.name.as_deref(),
        cmd.size.map(|s| s.as_str()),
        cmd.mass_status.map(|m| m.as_str()),
        cmd.time_status.map(|t| t.as_str()),
    )
    .fetch_one(pool)
    .await?;
    Ok(sig)
}

/// A partial edit of a signature's resolved name and wormhole state. `None` leaves a field
/// unchanged; `Some(None)` clears it. The group is fixed at add time (remove + re-add to
/// change it). Editing a linked wormhole's state propagates to its connection via the DB
/// trigger.
#[derive(Debug, Default, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateSignature {
    pub map_id: i64,
    pub signature_pk: i64,
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<Option<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub size: Option<Option<WormholeSize>>,
    #[serde(default)]
    #[ts(optional)]
    pub mass_status: Option<Option<MassStatus>>,
    #[serde(default)]
    #[ts(optional)]
    pub time_status: Option<Option<TimeStatus>>,
}

pub async fn update_signature(
    pool: &PgPool,
    actor: Actor,
    cmd: UpdateSignature,
) -> Result<Signature> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let current = fetch_signature(pool, cmd.map_id, cmd.signature_pk).await?;

    let name = cmd.name.unwrap_or(current.name);
    let size = cmd.size.unwrap_or(current.size);
    let mass = cmd.mass_status.unwrap_or(current.mass_status);
    let time = cmd.time_status.unwrap_or(current.time_status);

    if !is_wormhole(current.group) && (size.is_some() || mass.is_some() || time.is_some()) {
        return Err(MapError::Validation(
            "only a wormhole signature can carry size / mass / time".into(),
        ));
    }

    let sig = sqlx::query_as!(
        Signature,
        r#"update signatures set name = $1, size = $2, mass_status = $3, time_status = $4
           where id = $5 and map_id = $6
           returning id, map_id, solar_system_id, signature_id, "group" as "group: SignatureGroup",
                     name, size as "size: WormholeSize",
                     mass_status as "mass_status: MassStatus",
                     time_status as "time_status: TimeStatus",
                     connection_id, created_at, updated_at"#,
        name.as_deref(),
        size.map(|s| s.as_str()),
        mass.map(|m| m.as_str()),
        time.map(|t| t.as_str()),
        cmd.signature_pk,
        cmd.map_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(sig)
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct RemoveSignature {
    pub map_id: i64,
    pub signature_pk: i64,
}

/// Delete a signature. If it was linked, the connection survives (its other end too); the
/// hole's state simply loses this source.
pub async fn remove_signature(pool: &PgPool, actor: Actor, cmd: RemoveSignature) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let deleted = sqlx::query!(
        "delete from signatures where id = $1 and map_id = $2",
        cmd.signature_pk,
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
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let sig = fetch_signature(pool, cmd.map_id, cmd.signature_pk).await?;
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
    .fetch_one(pool)
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
    .execute(pool)
    .await?;

    // Re-read: the merge trigger may have changed this row's state after the UPDATE.
    fetch_signature(pool, cmd.map_id, cmd.signature_pk).await
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
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    let updated = sqlx::query!(
        "update signatures set connection_id = null where id = $1 and map_id = $2",
        cmd.signature_pk,
        cmd.map_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(MapError::NotFound);
    }
    fetch_signature(pool, cmd.map_id, cmd.signature_pk).await
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PastedSignature {
    pub signature_id: String,
    pub group: SignatureGroup,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PasteSignatures {
    pub map_id: i64,
    pub solar_system_id: i64,
    pub signatures: Vec<PastedSignature>,
}

/// Reconcile a pasted in-game scan against a system's signatures: add the new ones, refresh the
/// group/name of existing ones, and delete the ones that vanished from the scan — except linked
/// wormholes, which are kept (removing one would drop a mapped connection's end). A wormhole's
/// life-cycle state and connection link are never touched here.
pub async fn paste_signatures(pool: &PgPool, actor: Actor, cmd: PasteSignatures) -> Result<()> {
    require_role(pool, cmd.map_id, actor.user_id, Role::Member).await?;
    ensure_system_placed(pool, cmd.map_id, cmd.solar_system_id).await?;

    let mut tx = pool.begin().await?;
    let mut pasted_ids = std::collections::HashSet::new();
    for s in &cmd.signatures {
        let sid = s.signature_id.trim();
        if sid.is_empty() {
            continue;
        }
        pasted_ids.insert(sid.to_string());
        sqlx::query!(
            r#"insert into signatures (map_id, solar_system_id, signature_id, "group", name)
               values ($1, $2, $3, $4, $5)
               on conflict (map_id, solar_system_id, signature_id) do update
                   set "group" = excluded."group",
                       name = coalesce(excluded.name, signatures.name),
                       updated_at = now()"#,
            cmd.map_id,
            cmd.solar_system_id,
            sid,
            s.group.as_str(),
            s.name.as_deref(),
        )
        .execute(&mut *tx)
        .await?;
    }

    let existing = sqlx::query!(
        "select id, signature_id, connection_id from signatures
         where map_id = $1 and solar_system_id = $2",
        cmd.map_id,
        cmd.solar_system_id,
    )
    .fetch_all(&mut *tx)
    .await?;
    for row in existing {
        if !pasted_ids.contains(&row.signature_id) && row.connection_id.is_none() {
            sqlx::query!("delete from signatures where id = $1", row.id)
                .execute(&mut *tx)
                .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

/// Every signature on a map. Viewer+.
pub async fn list_signatures(pool: &PgPool, actor: Actor, map_id: i64) -> Result<Vec<Signature>> {
    require_role(pool, map_id, actor.user_id, Role::Viewer).await?;
    let sigs = sqlx::query_as!(
        Signature,
        r#"select id, map_id, solar_system_id, signature_id, "group" as "group: SignatureGroup",
                  name, size as "size: WormholeSize",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  connection_id, created_at, updated_at
           from signatures where map_id = $1 order by solar_system_id, signature_id"#,
        map_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(sigs)
}

/// Read one signature on a map, or `NotFound`.
async fn fetch_signature(pool: &PgPool, map_id: i64, signature_pk: i64) -> Result<Signature> {
    sqlx::query_as!(
        Signature,
        r#"select id, map_id, solar_system_id, signature_id, "group" as "group: SignatureGroup",
                  name, size as "size: WormholeSize",
                  mass_status as "mass_status: MassStatus",
                  time_status as "time_status: TimeStatus",
                  connection_id, created_at, updated_at
           from signatures where id = $1 and map_id = $2"#,
        signature_pk,
        map_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(MapError::NotFound)
}

/// `Validation` if the solar system isn't currently placed on the map (signatures hang off
/// a placement via the `(map_id, solar_system_id)` foreign key).
async fn ensure_system_placed(pool: &PgPool, map_id: i64, solar_system_id: i64) -> Result<()> {
    let placed = sqlx::query_scalar!(
        "select exists(select 1 from map_solar_systems where map_id = $1 and solar_system_id = $2)",
        map_id,
        solar_system_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);
    if !placed {
        return Err(MapError::Validation(
            "the signature's system is not placed on this map".into(),
        ));
    }
    Ok(())
}
