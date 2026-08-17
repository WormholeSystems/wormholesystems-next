//! Naming the things EVE only gives us ids for.
//!
//! Killmails, sovereignty and threat analysis all arrive as bare numbers, and all three
//! need to show a name. This is the one place that turns an id into a row: ask what we
//! already know, fetch only the rest, and keep it until it goes stale.
//!
//! Characters resolved here land in `characters` alongside the ones people sign in with,
//! without a `user_id`. That column is what separates the two: everything that cares about
//! whose character it is already filters on it.
//!
//! Nothing here belongs on an ingest path. Resolving is a background errand that runs
//! *after* the thing that needed it was recorded, so a slow or rate-limited ESI can never
//! hold up writing a killmail.

use std::collections::HashSet;

use sqlx::PgPool;

use crate::esi::EsiClient;
use crate::tracking::run_bounded;

/// Entries older than this are re-fetched: pilots change corp, corps change alliance.
const FRESH_FOR: &str = "7 days";

/// Max concurrent ESI lookups. Deliberately modest — this is never urgent.
const CONCURRENCY: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Character,
    Corporation,
    Alliance,
}

/// Make sure every id can be named, fetching the ones we cannot.
///
/// Unknown ids that ESI refuses (a deleted corp, a biomassed character) are simply left
/// unresolved and retried next time; there is no point failing a whole batch over one.
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

/// The subset of `ids` we cannot name, or last named too long ago.
pub async fn unresolved(pool: &PgPool, kind: EntityKind, ids: &HashSet<i64>) -> Vec<i64> {
    let ids: Vec<i64> = ids.iter().copied().collect();
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
    ids.into_iter().filter(|id| !fresh.contains(id)).collect()
}

async fn fetch(pool: PgPool, esi: EsiClient, kind: EntityKind, id: i64) {
    match kind {
        EntityKind::Character => fetch_character(pool, esi, id).await,
        EntityKind::Corporation => fetch_corporation(pool, esi, id).await,
        EntityKind::Alliance => fetch_alliance(pool, esi, id).await,
    }
}

async fn fetch_character(pool: PgPool, esi: EsiClient, id: i64) {
    let Ok(character) = esi.character_public(id).await else {
        return;
    };
    // Two things to be careful of. The upsert leaves `user_id` and `owner_hash` alone, so
    // resolving a name never disturbs whose login a character is. And the affiliations are
    // foreign keys, so an organisation we have not resolved yet is stored as null rather
    // than failing the whole insert and losing the name with it.
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
}

async fn fetch_corporation(pool: PgPool, esi: EsiClient, id: i64) {
    let Ok(corporation) = esi.corporation(id).await else {
        return;
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
}

async fn fetch_alliance(pool: PgPool, esi: EsiClient, id: i64) {
    let Ok(alliance) = esi.alliance(id).await else {
        return;
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
}

/// Documented here so the freshness window is stated once rather than inlined thrice.
pub const fn fresh_for() -> &'static str {
    FRESH_FOR
}
