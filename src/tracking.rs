//! Two-tier character-status polling — see
//! [processes.md](../docs/processes.md#character-status-polling).
//!
//! Single process, no job queue: tokio async concurrency handles the parallelism (these are
//! I/O-bound ESI calls), bounded by a [`Semaphore`] so we stay within ESI's error limit and
//! fit each tier's time budget. Tier 1 polls online state for the characters of active users
//! every 60s; tier 2 polls location + ship for the *online* ones every 5s.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::{MissedTickBehavior, interval};

use crate::db::PgTokenStore;
use crate::esi::scopes::Scope;
use crate::esi::{EsiClient, Sso};

/// Max ESI requests in flight per tick — the one tuning knob. Raise to fit more characters
/// in the 5s tier-2 budget; lower to stay further under ESI's error limit.
const CONCURRENCY: usize = 32;

/// Spawn the polling loops. Returns immediately; the loops run for the process lifetime.
pub fn start(pool: PgPool, sso: Arc<Sso>, esi: EsiClient) {
    tokio::spawn(tier_one(pool.clone(), sso.clone(), esi.clone()));
    tokio::spawn(tier_two(pool, sso, esi));
}

/// Tier 1: online state, every 60s, for every character of an active user.
async fn tier_one(pool: PgPool, sso: Arc<Sso>, esi: EsiClient) {
    let mut ticker = interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match active_characters(&pool, false).await {
            Ok(ids) => {
                run_bounded(&ids, |id| {
                    poll_online(pool.clone(), sso.clone(), esi.clone(), id)
                })
                .await
            }
            Err(err) => eprintln!("tracking tier-1 select failed: {err}"),
        }
    }
}

/// Tier 2: location + ship, every 5s, for currently-online characters of active users.
async fn tier_two(pool: PgPool, sso: Arc<Sso>, esi: EsiClient) {
    let mut ticker = interval(Duration::from_secs(5));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match active_characters(&pool, true).await {
            Ok(ids) => {
                run_bounded(&ids, |id| {
                    poll_location_ship(pool.clone(), sso.clone(), esi.clone(), id)
                })
                .await
            }
            Err(err) => eprintln!("tracking tier-2 select failed: {err}"),
        }
    }
}

/// Characters whose user has been active within the last 5 minutes. With `online_only`, also
/// restricted to those currently flagged online (the tier-2 set).
async fn active_characters(pool: &PgPool, online_only: bool) -> Result<Vec<i64>, sqlx::Error> {
    if online_only {
        sqlx::query_scalar!(
            "select c.id
             from characters c
             join users u on u.id = c.user_id
             join character_status s on s.character_id = c.id
             where u.last_active_at > now() - interval '5 minutes' and s.online"
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_scalar!(
            "select c.id
             from characters c
             join users u on u.id = c.user_id
             where u.last_active_at > now() - interval '5 minutes'"
        )
        .fetch_all(pool)
        .await
    }
}

/// Run `f` over every id with at most [`CONCURRENCY`] in flight; await the whole batch.
async fn run_bounded<F, Fut>(ids: &[i64], f: F)
where
    F: Fn(i64) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let permits = Arc::new(Semaphore::new(CONCURRENCY));
    let mut set = JoinSet::new();
    for &id in ids {
        // Acquiring before spawning is the backpressure: never more than CONCURRENCY tasks.
        let permit = permits
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore open");
        let fut = f(id);
        set.spawn(async move {
            let _permit = permit;
            fut.await;
        });
    }
    while set.join_next().await.is_some() {}
}

/// Poll one character's online state (tier 1). Errors (missing scope, ESI failure) skip the
/// character — the next tick retries.
async fn poll_online(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, character_id: i64) {
    let store = PgTokenStore::new(pool.clone());
    let Ok(token) = sso
        .access_token(&store, character_id, Scope::ReadOnline)
        .await
    else {
        return;
    };
    let Ok(status) = esi.character_online(&token, character_id).await else {
        return;
    };
    let _ = sqlx::query!(
        "insert into character_status (character_id, online, last_online_at)
         values ($1, $2, case when $2 then now() else null end)
         on conflict (character_id) do update set
             online = excluded.online,
             last_online_at = case when excluded.online then now()
                                   else character_status.last_online_at end,
             updated_at = now()",
        character_id,
        status.online,
    )
    .execute(&pool)
    .await;
}

/// Poll one character's location + ship (tier 2). Location and ship use independent scopes,
/// so a missing one skips just that field. Docking (station/structure) is left null until we
/// cache those entities — see the module note.
async fn poll_location_ship(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, character_id: i64) {
    let store = PgTokenStore::new(pool.clone());

    if let Ok(token) = sso
        .access_token(&store, character_id, Scope::ReadLocation)
        .await
        && let Ok(location) = esi.character_location(&token, character_id).await
    {
        let _ = sqlx::query!(
            "update character_status
             set solar_system_id = $2, station_id = null, structure_id = null, updated_at = now()
             where character_id = $1",
            character_id,
            location.solar_system_id,
        )
        .execute(&pool)
        .await;
    }

    if let Ok(token) = sso
        .access_token(&store, character_id, Scope::ReadShipType)
        .await
        && let Ok(ship) = esi.character_ship(&token, character_id).await
    {
        let _ = sqlx::query!(
            "update character_status
             set ship_type_id = $2, ship_name = $3, ship_item_id = $4,
                 ship_updated_at = case when ship_item_id is distinct from $4 then now()
                                        else ship_updated_at end,
                 updated_at = now()
             where character_id = $1",
            character_id,
            ship.ship_type_id,
            ship.ship_name,
            ship.ship_item_id,
        )
        .execute(&pool)
        .await;
    }
}
