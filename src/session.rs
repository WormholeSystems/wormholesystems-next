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

/// An ESI entity (corp/alliance) we cache so a character's deferred FKs resolve. Carries
/// just what the entity tables require; refreshed on each login.
pub struct Entity {
    pub id: i64,
    pub name: String,
    pub ticker: String,
}

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

/// Open a 30-day session for a user. Returns the opaque id to put in the cookie.
///
/// The session starts as the user's preferred character rather than as `character_id`, the
/// one that just signed in: which alt the SSO happened to hand back is not a choice the
/// user made, and the preferred one is. It only falls back to `character_id` for a user
/// with no preference at all.
pub async fn create_session(
    pool: &PgPool,
    user_id: i64,
    character_id: i64,
) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        "insert into sessions (id, user_id, active_character_id, expires_at)
         values ($1, $2,
                 coalesce((select id from characters where user_id = $2 and is_preferred), $3),
                 now() + interval '30 days')",
        id,
        user_id,
        character_id,
    )
    .execute(pool)
    .await?;
    Ok(id)
}

/// Mark the user as active *now* — gates which characters the tracking poller polls
/// (see [processes.md](../docs/processes.md#character-status-polling)). Driven by the
/// per-user WebSocket heartbeat.
pub async fn touch_activity(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "update users set last_active_at = now() where id = $1",
        user_id,
    )
    .execute(pool)
    .await?;
    Ok(())
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

/// Choose the character new sessions start as. Verifies it belongs to `user_id`, and
/// clears the old preference first: at most one per user is a unique index, not a
/// convention.
pub async fn set_preferred_character(
    pool: &PgPool,
    user_id: i64,
    character_id: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "update characters set is_preferred = false where user_id = $1 and is_preferred",
        user_id,
    )
    .execute(&mut *tx)
    .await?;
    let updated = sqlx::query!(
        "update characters set is_preferred = true where id = $1 and user_id = $2",
        character_id,
        user_id,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    tx.commit().await?;
    Ok(true)
}

/// Give a user a preferred character if they have none, so removing the preferred one does
/// not leave the account without a default. Lowest character id wins; there is nothing
/// better to go on.
pub async fn ensure_preferred_character(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "update characters set is_preferred = true
         where id = (select id from characters where user_id = $1 order by id limit 1)
           and not exists (select 1 from characters where user_id = $1 and is_preferred)",
        user_id,
    )
    .execute(pool)
    .await?;
    Ok(())
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
    corporation: Entity,
    alliance: Option<Entity>,
    link_user_id: Option<i64>,
) -> Result<i64, sqlx::Error> {
    let character_id = claims.character_id;
    let alliance_id = alliance.as_ref().map(|a| a.id);
    let mut tx = pool.begin().await?;

    // The character's corp/alliance are deferred FKs to the ESI-cached entity tables, so
    // ensure those rows exist (and are kept fresh) before/with the character row.
    sqlx::query!(
        "insert into corporations (id, name, ticker) values ($1, $2, $3)
         on conflict (id) do update set name = excluded.name, ticker = excluded.ticker, updated_at = now()",
        corporation.id,
        corporation.name,
        corporation.ticker,
    )
    .execute(&mut *tx)
    .await?;
    if let Some(alliance) = &alliance {
        sqlx::query!(
            "insert into alliances (id, name, ticker) values ($1, $2, $3)
             on conflict (id) do update set name = excluded.name, ticker = excluded.ticker, updated_at = now()",
            alliance.id,
            alliance.name,
            alliance.ticker,
        )
        .execute(&mut *tx)
        .await?;
    }

    let existing = sqlx::query!(
        "select user_id, owner_hash from characters where id = $1",
        character_id,
    )
    .fetch_optional(&mut *tx)
    .await?;

    let user_id = match (link_user_id, existing) {
        // Linking a character to the already-signed-in user.
        (Some(user_id), _) => user_id,
        // Returning login: same owner hash → same account. A character we merely resolved
        // the name of has neither, and falls through to a fresh account like any new one.
        (None, Some(c))
            if c.owner_hash.as_deref() == Some(claims.owner_hash.as_str())
                && c.user_id.is_some() =>
        {
            c.user_id.expect("guarded above")
        }
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
        corporation.id,
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
