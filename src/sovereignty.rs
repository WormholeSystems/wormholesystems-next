//! Sovereignty sync — keeps `system_sovereignty` (and the alliance/corp entities it names)
//! current so map nodes can show human-readable holders.
//!
//! Single periodic loop, like [`tracking`](crate::tracking) but simpler: the sovereignty +
//! corporation/alliance ESI endpoints are **public** (no token, no scopes, no per-character
//! work), so it needs only the pool and an [`EsiClient`]. Each tick fetches the full
//! sovereignty map in one call, fetches any holder entity we don't already have (or haven't
//! refreshed in a week), and upserts the per-system holder. Factions come from the SDE, so
//! they're never fetched.

use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::time::{MissedTickBehavior, interval};

use crate::esi::EsiClient;
use crate::esi::sovereignty::SovereigntySystem;
use crate::tracking::run_bounded;

/// How often to refresh sovereignty. It changes slowly (alliance-level territory), so an hour
/// is gentle on ESI while keeping the map reasonably fresh.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Max concurrent entity fetches per tick.
const CONCURRENCY: usize = 16;

/// Spawn the sync loop. Returns immediately; the loop runs for the process lifetime.
pub fn start(pool: PgPool, esi: EsiClient) {
    tokio::spawn(sync_loop(pool, esi));
}

async fn sync_loop(pool: PgPool, esi: EsiClient) {
    let mut ticker = interval(INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        sync_once(&pool, &esi).await;
    }
}

async fn sync_once(pool: &PgPool, esi: &EsiClient) {
    let systems = match esi.sovereignty_systems().await {
        Ok(systems) => systems,
        Err(err) => {
            eprintln!("sovereignty fetch failed: {err}");
            return;
        }
    };

    // Resolve holder entities first so the per-system upserts below don't trip the FKs.
    resolve_entities(pool, esi, &systems).await;

    for system in &systems {
        if let Err(err) = upsert_system(pool, system).await {
            // A system we don't have in the SDE, or an entity that failed to resolve — skip
            // it; the next tick retries.
            eprintln!(
                "sovereignty upsert for system {} skipped: {err}",
                system.solar_system_id
            );
        }
    }
}

/// Fetch + upsert every alliance and corporation named by a claim that we don't already have
/// fresh. Failures are skipped (the next tick retries); factions are SDE-seeded, never fetched.
async fn resolve_entities(pool: &PgPool, esi: &EsiClient, systems: &[SovereigntySystem]) {
    let mut alliance_ids = HashSet::new();
    let mut corporation_ids = HashSet::new();
    for system in systems {
        if let Some(claim) = &system.claim.alliance {
            alliance_ids.insert(claim.alliance_id);
            corporation_ids.insert(claim.corporation_id);
        }
    }

    let alliances = stale_ids(pool, EntityKind::Alliance, &alliance_ids).await;
    run_bounded(&alliances, CONCURRENCY, |id| {
        fetch_alliance(pool.clone(), esi.clone(), id)
    })
    .await;

    let corporations = stale_ids(pool, EntityKind::Corporation, &corporation_ids).await;
    run_bounded(&corporations, CONCURRENCY, |id| {
        fetch_corporation(pool.clone(), esi.clone(), id)
    })
    .await;
}

#[derive(Clone, Copy)]
enum EntityKind {
    Alliance,
    Corporation,
}

/// Of `ids`, the ones we should (re)fetch: missing entirely, or refreshed over a week ago.
async fn stale_ids(pool: &PgPool, kind: EntityKind, ids: &HashSet<i64>) -> Vec<i64> {
    let ids: Vec<i64> = ids.iter().copied().collect();
    let fresh = match kind {
        EntityKind::Alliance => {
            sqlx::query_scalar!(
                "select id from alliances
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
    };
    let fresh: HashSet<i64> = match fresh {
        Ok(rows) => rows.into_iter().collect(),
        Err(err) => {
            eprintln!("sovereignty entity freshness check failed: {err}");
            return Vec::new();
        }
    };
    ids.into_iter().filter(|id| !fresh.contains(id)).collect()
}

async fn fetch_alliance(pool: PgPool, esi: EsiClient, id: i64) {
    let Ok(alliance) = esi.alliance(id).await else {
        return;
    };
    let _ = sqlx::query!(
        "insert into alliances (id, name, ticker) values ($1, $2, $3)
         on conflict (id) do update set
             name = excluded.name, ticker = excluded.ticker, updated_at = now()",
        id,
        alliance.name,
        alliance.ticker,
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
         values ($1, $2, $3, $4, $5)
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

/// Upsert one system's holder: alliance, faction, or none (unclaimed → remove any row).
async fn upsert_system(pool: &PgPool, system: &SovereigntySystem) -> Result<(), sqlx::Error> {
    let claim = &system.claim;
    if let Some(alliance) = &claim.alliance {
        let claimed_since = alliance.claimed_since.as_deref().and_then(parse_timestamp);
        sqlx::query!(
            "insert into system_sovereignty
                 (solar_system_id, alliance_id, corporation_id, claimed_since, is_capital_system)
             values ($1, $2, $3, $4, $5)
             on conflict (solar_system_id) do update set
                 alliance_id = excluded.alliance_id, corporation_id = excluded.corporation_id,
                 faction_id = null, claimed_since = excluded.claimed_since,
                 is_capital_system = excluded.is_capital_system, updated_at = now()",
            system.solar_system_id,
            alliance.alliance_id,
            alliance.corporation_id,
            claimed_since,
            alliance.is_capital_system,
        )
        .execute(pool)
        .await?;
    } else if let Some(faction) = &claim.faction {
        sqlx::query!(
            "insert into system_sovereignty (solar_system_id, faction_id) values ($1, $2)
             on conflict (solar_system_id) do update set
                 faction_id = excluded.faction_id, alliance_id = null, corporation_id = null,
                 claimed_since = null, is_capital_system = null, updated_at = now()",
            system.solar_system_id,
            faction.faction_id,
        )
        .execute(pool)
        .await?;
    } else {
        sqlx::query!(
            "delete from system_sovereignty where solar_system_id = $1",
            system.solar_system_id,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// ESI timestamps are RFC 3339; drop any that don't parse rather than failing the row.
fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}
