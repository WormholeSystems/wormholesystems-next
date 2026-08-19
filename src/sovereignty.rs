//! Sovereignty sync: keeps `system_sovereignty` (and the alliance/corp entities it names)
//! current so map nodes can show human-readable holders.
//!
//! The endpoints involved are public, so this needs no token and no per-character work. Each
//! tick fetches the whole sovereignty map in one call, resolves any holder entity we don't
//! have fresh, and upserts the per-system holder. Factions come from the SDE, never ESI.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::time::{MissedTickBehavior, interval};

use crate::entities::{self, EntityKind};
use crate::esi::EsiClient;
use crate::esi::sovereignty::SovereigntySystem;
use crate::server_status::ServerWatch;

/// How often to refresh sovereignty. It changes slowly (alliance-level territory), so an hour
/// is gentle on ESI while keeping the map reasonably fresh.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Spawn the sync loop. Returns immediately; the loop runs for the process lifetime.
pub fn start(pool: PgPool, esi: EsiClient, server: ServerWatch) {
    tokio::spawn(sync_loop(pool, esi, server));
}

async fn sync_loop(pool: PgPool, esi: EsiClient, server: ServerWatch) {
    let mut ticker = interval(INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        // Sovereignty does not move while nobody is playing, and the call would fail
        // anyway. The next tick picks it up once Tranquility is back.
        if !server.should_poll() {
            continue;
        }
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
            // A system we don't have in the SDE, or an entity that failed to resolve. Skip
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
    let mut alliance_ids = Vec::new();
    let mut corporation_ids = Vec::new();
    for system in systems {
        if let Some(claim) = &system.claim.alliance {
            alliance_ids.push(claim.alliance_id);
            corporation_ids.push(claim.corporation_id);
        }
    }
    entities::ensure(pool, esi, EntityKind::Alliance, &alliance_ids).await;
    entities::ensure(pool, esi, EntityKind::Corporation, &corporation_ids).await;
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
