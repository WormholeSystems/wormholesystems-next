//! Authorization helpers and the access-management actions (`set_access`,
//! `revoke_access`). See [access.md](../../docs/database/access.md).

use sqlx::PgPool;

use super::error::{MapError, Result};
use super::{Actor, Role, SubjectType};

/// The user's effective role on a map: the highest role they match across *all* their
/// characters (a character's own id, its corporation, or its alliance). `None` means no
/// access at all.
pub async fn effective_role(pool: &PgPool, map_id: i64, user_id: i64) -> Result<Option<Role>> {
    let roles = sqlx::query_scalar!(
        r#"select role as "role!: Role"
           from map_access
           where map_id = $1
             and subject_id in (
                 select id from characters where user_id = $2
                 union all
                 select corporation_id from characters where user_id = $2
                 union all
                 select alliance_id from characters where user_id = $2 and alliance_id is not null
             )"#,
        map_id,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(roles.into_iter().max())
}

/// Require at least `min` on the map. Maps the role gap to the canonical errors: no
/// access at all → `NotFound` (don't reveal the map); access but too low → `Forbidden`.
/// Returns the actor's actual effective role on success (callers use it for the grant
/// privilege ceiling).
pub(super) async fn require_role(
    pool: &PgPool,
    map_id: i64,
    user_id: i64,
    min: Role,
) -> Result<Role> {
    match effective_role(pool, map_id, user_id).await? {
        None => Err(MapError::NotFound),
        Some(role) if role >= min => Ok(role),
        Some(_) => Err(MapError::Forbidden),
    }
}

/// Whether `character_id` belongs to `user_id`.
pub(super) async fn owns_character(pool: &PgPool, user_id: i64, character_id: i64) -> Result<bool> {
    let exists = sqlx::query_scalar!(
        "select exists(select 1 from characters where id = $1 and user_id = $2)",
        character_id,
        user_id,
    )
    .fetch_one(pool)
    .await?;
    Ok(exists.unwrap_or(false))
}

/// Grant `role` to a subject, or change an existing subject's role. Manager+, and you
/// may not grant a role above your own. Never leaves the map without an owner.
pub async fn set_access(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
    subject_type: SubjectType,
    subject_id: i64,
    role: Role,
) -> Result<()> {
    let actor_role = require_role(pool, map_id, actor.user_id, Role::Manager).await?;
    if role > actor_role {
        return Err(MapError::Forbidden);
    }

    let mut tx = pool.begin().await?;
    sqlx::query!(
        "insert into map_access (map_id, subject_type, subject_id, role)
         values ($1, $2, $3, $4)
         on conflict (map_id, subject_id)
         do update set subject_type = excluded.subject_type, role = excluded.role",
        map_id,
        subject_type.as_str(),
        subject_id,
        role.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    ensure_has_owner(&mut tx, map_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Remove a subject's grant. Manager+. Revoking the last owner is rejected.
pub async fn revoke_access(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
    subject_id: i64,
) -> Result<()> {
    require_role(pool, map_id, actor.user_id, Role::Manager).await?;

    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        "delete from map_access where map_id = $1 and subject_id = $2",
        map_id,
        subject_id,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if deleted == 0 {
        return Err(MapError::NotFound);
    }
    ensure_has_owner(&mut tx, map_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Fail (rolling back the transaction) if the map has no owner left.
async fn ensure_has_owner(tx: &mut sqlx::PgConnection, map_id: i64) -> Result<()> {
    let owners = sqlx::query_scalar!(
        "select count(*) from map_access where map_id = $1 and role = 'owner'",
        map_id,
    )
    .fetch_one(&mut *tx)
    .await?
    .unwrap_or(0);
    if owners == 0 {
        return Err(MapError::LastOwner);
    }
    Ok(())
}
