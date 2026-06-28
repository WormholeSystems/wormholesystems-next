//! App sessions and the SSO login → identity resolution.
//!
//! A login authenticates a *character*; we resolve it to one of our [`users`] accounts per
//! [authentication.md](../docs/database/authentication.md), persist the character + token,
//! and open a [`sessions`] row. The cookie carries only the opaque session id; the active
//! character is per-session. Expiry is enforced in SQL (`now()`), so this never needs
//! chrono's clock.

use sqlx::PgPool;
use uuid::Uuid;

use crate::esi::jwt::Claims;
use crate::maps::Actor;

/// Name of the cookie holding the opaque session id.
pub const SESSION_COOKIE: &str = "vector_session";

/// Resolve a session id to the acting user + their active character, or `None` if the
/// session is unknown or expired.
pub async fn actor_for_session(
    pool: &PgPool,
    session_id: &str,
) -> Result<Option<Actor>, sqlx::Error> {
    let row = sqlx::query!(
        "select user_id, active_character_id
         from sessions where id = $1 and expires_at > now()",
        session_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Actor {
        user_id: r.user_id,
        character_id: r.active_character_id,
    }))
}

/// Open a 30-day session for a user acting as `character_id`. Returns the opaque id to put
/// in the cookie.
pub async fn create_session(
    pool: &PgPool,
    user_id: i64,
    character_id: i64,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        "insert into sessions (id, user_id, active_character_id, expires_at)
         values ($1, $2, $3, now() + interval '30 days')",
        id,
        user_id,
        character_id,
    )
    .execute(pool)
    .await?;
    Ok(id)
}

/// End a session (logout).
pub async fn delete_session(pool: &PgPool, session_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("delete from sessions where id = $1", session_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Change the active character of a session (the per-session character switcher). Verifies
/// the character belongs to the session's user.
pub async fn set_active_character(
    pool: &PgPool,
    session_id: &str,
    character_id: i64,
) -> Result<bool, sqlx::Error> {
    let updated = sqlx::query!(
        "update sessions s set active_character_id = $2
         where s.id = $1
           and exists (select 1 from characters c where c.id = $2 and c.user_id = s.user_id)",
        session_id,
        character_id,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(updated > 0)
}

/// Resolve the SSO login to a user and persist the character. Returns the owning `user_id`
/// (the character id is `claims.character_id`). Implements the identity rules from the spec:
/// returning login reuses the user; an **owner-hash change** (character transfer) reassigns
/// the character to a fresh user; a brand-new character creates a user; `link_user_id`
/// attaches the character to an already-signed-in user. The first character of a user
/// becomes its preferred one.
pub async fn persist_identity(
    pool: &PgPool,
    claims: &Claims,
    corporation_id: i64,
    alliance_id: Option<i64>,
    link_user_id: Option<i64>,
) -> Result<i64, sqlx::Error> {
    let character_id = claims.character_id;
    let mut tx = pool.begin().await?;

    let existing = sqlx::query!(
        "select user_id, owner_hash from characters where id = $1",
        character_id,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let user_id = match (link_user_id, existing) {
        // Linking a character to the already-signed-in user.
        (Some(user_id), _) => user_id,
        // Returning login: same owner hash → same account.
        (None, Some(c)) if c.owner_hash == claims.owner_hash => c.user_id,
        // Transfer (owner hash changed) or brand-new character → a fresh account.
        (None, _) => {
            sqlx::query_scalar!("insert into users default values returning id")
                .fetch_one(&mut *tx)
                .await?
        }
    };

    // Upsert the character (this reassigns `user_id` on transfer / link). Corp + alliance
    // are refreshed from ESI on every login since they drive access checks.
    sqlx::query!(
        "insert into characters (id, user_id, name, owner_hash, corporation_id, alliance_id)
         values ($1, $2, $3, $4, $5, $6)
         on conflict (id) do update set
             user_id = excluded.user_id,
             name = excluded.name,
             owner_hash = excluded.owner_hash,
             corporation_id = excluded.corporation_id,
             alliance_id = excluded.alliance_id,
             updated_at = now()",
        character_id,
        user_id,
        claims.name,
        claims.owner_hash,
        corporation_id,
        alliance_id,
    )
    .execute(&mut *tx)
    .await?;

    // The first character a user has becomes its preferred one (seeds new sessions).
    sqlx::query!(
        "update characters set is_preferred = true
         where id = $1
           and not exists (select 1 from characters where user_id = $2 and is_preferred)",
        character_id,
        user_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(user_id)
}
