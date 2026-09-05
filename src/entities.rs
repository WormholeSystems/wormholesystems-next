//! Naming the things EVE only gives us ids for.
//!
//! Characters resolved here land in `characters` alongside the ones people sign in with,
//! but without a `user_id`. That column is what separates the two.
//!
//! Nothing here belongs on an ingest path. Resolving is a background errand that runs after
//! the thing that needed it was recorded, so a slow or rate-limited ESI can never hold up
//! writing a killmail.

use std::collections::HashSet;

use sqlx::PgPool;

use crate::esi::{EsiClient, EsiError};
use crate::tracking::run_bounded;

/// Entries older than this are re-fetched: pilots change corp, corps change alliance.
const FRESH_FOR: &str = "7 days";

/// Max concurrent ESI lookups. Deliberately modest, since this is never urgent.
const CONCURRENCY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Character,
    Corporation,
    Alliance,
}

impl EntityKind {
    /// The `kind` column of `unresolvable_entities`.
    fn as_str(self) -> &'static str {
        match self {
            EntityKind::Character => "character",
            EntityKind::Corporation => "corporation",
            EntityKind::Alliance => "alliance",
        }
    }
}

/// Make sure every id can be named, fetching the ones we cannot.
///
/// An id ESI refuses with a 404 (a closed corp, a biomassed character) is noted in
/// `unresolvable_entities` and skipped for a month, since a row without a name cannot say
/// so itself. Any other failure is simply retried next time; there is no point failing a
/// whole batch over one.
pub async fn ensure(pool: &PgPool, esi: &EsiClient, kind: EntityKind, ids: &[i64]) {
    let wanted: HashSet<i64> = ids.iter().copied().filter(|id| *id > 0).collect();
    if wanted.is_empty() {
        return;
    }
    let missing = unresolved(pool, kind, &wanted).await;
    run_bounded(&missing, CONCURRENCY, |id| {
        fetch(pool.clone(), esi.clone(), kind, id)
    })
    .await;
}

/// The subset of `ids` we cannot name, or last named too long ago, minus the ones ESI
/// recently said do not exist.
pub async fn unresolved(pool: &PgPool, kind: EntityKind, ids: &HashSet<i64>) -> Vec<i64> {
    let ids: Vec<i64> = ids.iter().copied().collect();
    let dead = sqlx::query_scalar!(
        "select id from unresolvable_entities
         where kind = $1 and id = any($2) and noted_at > now() - interval '30 days'",
        kind.as_str(),
        &ids,
    )
    .fetch_all(pool)
    .await;
    let dead: HashSet<i64> = match dead {
        Ok(rows) => rows.into_iter().collect(),
        Err(err) => {
            eprintln!("unresolvable check failed: {err}");
            HashSet::new()
        }
    };
    // One query per kind rather than a dynamic table name: `query!` checks these at
    // compile time, and there are only three.
    let fresh = match kind {
        EntityKind::Character => {
            sqlx::query_scalar!(
                "select id from characters
                 where id = any($1) and updated_at > now() - interval '7 days'",
                &ids,
            )
            .fetch_all(pool)
            .await
        }
        EntityKind::Corporation => {
            sqlx::query_scalar!(
                "select id from corporations
                 where id = any($1) and updated_at > now() - interval '7 days'",
                &ids,
            )
            .fetch_all(pool)
            .await
        }
        EntityKind::Alliance => {
            sqlx::query_scalar!(
                "select id from alliances
                 where id = any($1) and updated_at > now() - interval '7 days'",
                &ids,
            )
            .fetch_all(pool)
            .await
        }
    };
    let fresh: HashSet<i64> = match fresh {
        Ok(rows) => rows.into_iter().collect(),
        Err(err) => {
            eprintln!("entity freshness check failed: {err}");
            return Vec::new();
        }
    };
    ids.into_iter()
        .filter(|id| !fresh.contains(id) && !dead.contains(id))
        .collect()
}

/// Remember that ESI has no such thing, so the next pass does not ask again.
async fn note_unresolvable(pool: &PgPool, kind: EntityKind, id: i64) {
    let _ = sqlx::query!(
        "insert into unresolvable_entities (kind, id) values ($1, $2)
         on conflict (kind, id) do update set noted_at = now()",
        kind.as_str(),
        id,
    )
    .execute(pool)
    .await;
}

/// The id is back, or never was gone: whatever we noted no longer holds.
async fn forget_unresolvable(pool: &PgPool, kind: EntityKind, id: i64) {
    let _ = sqlx::query!(
        "delete from unresolvable_entities where kind = $1 and id = $2",
        kind.as_str(),
        id,
    )
    .execute(pool)
    .await;
}

/// Whether a failed fetch is ESI saying the id does not exist, rather than ESI being
/// unavailable or slow.
fn is_not_found(err: &EsiError) -> bool {
    matches!(err, EsiError::Api { status: 404, .. })
}

/// Name a large batch of characters in as few calls as possible.
///
/// The bulk endpoint takes a thousand ids at a time and returns only names, which is all a
/// killmail row asks of a character; the per-character endpoint is one request each, which
/// is hopeless for the tens of thousands in a year of history.
pub async fn ensure_character_names(pool: &PgPool, esi: &EsiClient, ids: &[i64]) -> usize {
    let wanted: HashSet<i64> = ids.iter().copied().filter(|id| *id > 0).collect();
    if wanted.is_empty() {
        return 0;
    }
    let missing = unresolved(pool, EntityKind::Character, &wanted).await;
    let mut named = 0usize;
    for chunk in missing.chunks(1000) {
        let Ok(rows) = esi.universe_names(chunk).await else {
            // A batch containing one dead id fails as a whole. Skipping it costs those
            // names until the next run rather than the whole import.
            continue;
        };
        let (ids, names): (Vec<i64>, Vec<String>) = rows
            .into_iter()
            .filter(|r| r.category == "character")
            .map(|r| (r.id, r.name))
            .collect();
        if ids.is_empty() {
            continue;
        }
        // Name only: this must never disturb whose login a character is, nor overwrite an
        // affiliation the richer per-character fetch has already established.
        let written = sqlx::query!(
            "insert into characters (id, name)
             select * from unnest($1::bigint[], $2::text[])
             on conflict (id) do update set name = excluded.name, updated_at = now()",
            &ids,
            &names,
        )
        .execute(pool)
        .await
        .map(|r| r.rows_affected() as usize)
        .unwrap_or(0);
        named += written;
    }
    named
}

async fn fetch(pool: PgPool, esi: EsiClient, kind: EntityKind, id: i64) {
    match kind {
        EntityKind::Character => fetch_character(pool, esi, id).await,
        EntityKind::Corporation => fetch_corporation(pool, esi, id).await,
        EntityKind::Alliance => fetch_alliance(pool, esi, id).await,
    }
}

async fn fetch_character(pool: PgPool, esi: EsiClient, id: i64) {
    let character = match esi.character_public(id).await {
        Ok(character) => character,
        Err(err) => {
            if is_not_found(&err) {
                note_unresolvable(&pool, EntityKind::Character, id).await;
            }
            return;
        }
    };
    // The upsert leaves `user_id` and `owner_hash` alone, so resolving a name never disturbs
    // whose login a character is. Affiliations are foreign keys, so an organisation we have
    // not resolved yet is stored as null rather than failing the insert and losing the name.
    let _ = sqlx::query!(
        "insert into characters (id, name, corporation_id, alliance_id)
         values ($1, $2,
                 (select id from corporations where id = $3),
                 (select id from alliances where id = $4))
         on conflict (id) do update set
             name = excluded.name,
             corporation_id = excluded.corporation_id,
             alliance_id = excluded.alliance_id,
             updated_at = now()",
        id,
        character.name,
        character.corporation_id,
        character.alliance_id,
    )
    .execute(&pool)
    .await;
    forget_unresolvable(&pool, EntityKind::Character, id).await;
}

async fn fetch_corporation(pool: PgPool, esi: EsiClient, id: i64) {
    let corporation = match esi.corporation(id).await {
        Ok(corporation) => corporation,
        Err(err) => {
            if is_not_found(&err) {
                note_unresolvable(&pool, EntityKind::Corporation, id).await;
            }
            return;
        }
    };
    let _ = sqlx::query!(
        "insert into corporations (id, name, ticker, alliance_id, faction_id)
         values ($1, $2, $3, (select id from alliances where id = $4),
                 (select id from factions where id = $5))
         on conflict (id) do update set
             name = excluded.name, ticker = excluded.ticker,
             alliance_id = excluded.alliance_id, faction_id = excluded.faction_id,
             updated_at = now()",
        id,
        corporation.name,
        corporation.ticker,
        corporation.alliance_id,
        corporation.faction_id,
    )
    .execute(&pool)
    .await;
    forget_unresolvable(&pool, EntityKind::Corporation, id).await;
}

async fn fetch_alliance(pool: PgPool, esi: EsiClient, id: i64) {
    let alliance = match esi.alliance(id).await {
        Ok(alliance) => alliance,
        Err(err) => {
            if is_not_found(&err) {
                note_unresolvable(&pool, EntityKind::Alliance, id).await;
            }
            return;
        }
    };
    let _ = sqlx::query!(
        "insert into alliances
             (id, name, ticker, creator_corporation_id, executor_corporation_id, faction_id)
         values ($1, $2, $3,
                 (select id from corporations where id = $4),
                 (select id from corporations where id = $5),
                 (select id from factions where id = $6))
         on conflict (id) do update set
             name = excluded.name, ticker = excluded.ticker,
             creator_corporation_id = excluded.creator_corporation_id,
             executor_corporation_id = excluded.executor_corporation_id,
             faction_id = excluded.faction_id,
             updated_at = now()",
        id,
        alliance.name,
        alliance.ticker,
        alliance.creator_corporation_id,
        alliance.executor_corporation_id,
        alliance.faction_id,
    )
    .execute(&pool)
    .await;
    forget_unresolvable(&pool, EntityKind::Alliance, id).await;
}

pub const fn fresh_for() -> &'static str {
    FRESH_FOR
}
