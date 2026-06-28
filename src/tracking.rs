//! Two-tier character-status polling — see
//! [processes.md](../docs/processes.md#character-status-polling).
//!
//! Single process, no job queue: tokio async concurrency handles the parallelism (these are
//! I/O-bound ESI calls), bounded by a [`Semaphore`] so we stay within ESI's error limit and
//! fit each tier's time budget. Tier 1 polls online state for the characters of active users
//! every 60s; tier 2 polls location + ship for the *online* ones every 5s. After a poll
//! updates a character, we ping its user's private channel so their UI refetches.

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
use crate::user_channel::{UserEvent, UserHub};

/// Max ESI requests in flight per tick — the one tuning knob. Raise to fit more characters
/// in the 5s tier-2 budget; lower to stay further under ESI's error limit.
const CONCURRENCY: usize = 32;

/// A character due for polling, with the user to notify.
#[derive(Clone, Copy)]
struct Due {
    character_id: i64,
    user_id: i64,
}

/// Spawn the polling loops. Returns immediately; the loops run for the process lifetime.
pub fn start(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, users: UserHub) {
    tokio::spawn(tier_one(
        pool.clone(),
        sso.clone(),
        esi.clone(),
        users.clone(),
    ));
    tokio::spawn(tier_two(pool, sso, esi, users));
}

/// Tier 1: online state, every 60s, for every character of an active user.
async fn tier_one(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, users: UserHub) {
    let mut ticker = interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match active_characters(&pool, false).await {
            Ok(due) => {
                run_bounded(&due, CONCURRENCY, |d| {
                    poll_online(pool.clone(), sso.clone(), esi.clone(), users.clone(), d)
                })
                .await
            }
            Err(err) => eprintln!("tracking tier-1 select failed: {err}"),
        }
    }
}

/// Tier 2: location + ship, every 5s, for currently-online characters of active users.
async fn tier_two(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, users: UserHub) {
    let mut ticker = interval(Duration::from_secs(5));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match active_characters(&pool, true).await {
            Ok(due) => {
                run_bounded(&due, CONCURRENCY, |d| {
                    poll_location_ship(pool.clone(), sso.clone(), esi.clone(), users.clone(), d)
                })
                .await
            }
            Err(err) => eprintln!("tracking tier-2 select failed: {err}"),
        }
    }
}

/// Characters whose user has been active within the last 5 minutes. With `online_only`, also
/// restricted to those currently flagged online (the tier-2 set).
async fn active_characters(pool: &PgPool, online_only: bool) -> Result<Vec<Due>, sqlx::Error> {
    let rows = if online_only {
        sqlx::query!(
            "select c.id, c.user_id
             from characters c
             join users u on u.id = c.user_id
             join character_status s on s.character_id = c.id
             where u.last_active_at > now() - interval '5 minutes' and s.online"
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| Due {
            character_id: r.id,
            user_id: r.user_id,
        })
        .collect()
    } else {
        sqlx::query!(
            "select c.id, c.user_id
             from characters c
             join users u on u.id = c.user_id
             where u.last_active_at > now() - interval '5 minutes'"
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| Due {
            character_id: r.id,
            user_id: r.user_id,
        })
        .collect()
    };
    Ok(rows)
}

/// Run `f` over every item with at most `concurrency` in flight; await the whole batch.
pub(crate) async fn run_bounded<T, F, Fut>(items: &[T], concurrency: usize, f: F)
where
    T: Copy + Send + 'static,
    F: Fn(T) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let permits = Arc::new(Semaphore::new(concurrency));
    let mut set = JoinSet::new();
    for &item in items {
        // Acquiring before spawning is the backpressure: never more than CONCURRENCY tasks.
        let permit = permits
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore open");
        let fut = f(item);
        set.spawn(async move {
            let _permit = permit;
            fut.await;
        });
    }
    while set.join_next().await.is_some() {}
}

/// Poll one character's online state (tier 1). Errors (missing scope, ESI failure) skip the
/// character — the next tick retries.
async fn poll_online(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, users: UserHub, due: Due) {
    let store = PgTokenStore::new(pool.clone());
    let Ok(token) = sso
        .access_token(&store, due.character_id, Scope::ReadOnline)
        .await
    else {
        return;
    };
    let Ok(status) = esi.character_online(&token, due.character_id).await else {
        return;
    };
    let updated = sqlx::query!(
        "insert into character_status (character_id, online, last_online_at)
         values ($1, $2, case when $2 then now() else null end)
         on conflict (character_id) do update set
             online = excluded.online,
             last_online_at = case when excluded.online then now()
                                   else character_status.last_online_at end,
             updated_at = now()",
        due.character_id,
        status.online,
    )
    .execute(&pool)
    .await
    .is_ok();
    if updated {
        users.publish(
            due.user_id,
            UserEvent::CharacterStatusChanged {
                character_id: due.character_id,
            },
        );
    }
}

/// Poll one character's location + ship (tier 2). Location and ship use independent scopes,
/// so a missing one skips just that field. Docking (station/structure) is left null until we
/// cache those entities — see the module note.
async fn poll_location_ship(pool: PgPool, sso: Arc<Sso>, esi: EsiClient, users: UserHub, due: Due) {
    let store = PgTokenStore::new(pool.clone());
    let id = due.character_id;
    let mut changed = false;

    if let Ok(token) = sso.access_token(&store, id, Scope::ReadLocation).await
        && let Ok(location) = esi.character_location(&token, id).await
    {
        changed |= sqlx::query!(
            "update character_status
             set solar_system_id = $2, station_id = null, structure_id = null, updated_at = now()
             where character_id = $1",
            id,
            location.solar_system_id,
        )
        .execute(&pool)
        .await
        .is_ok();
    }

    if let Ok(token) = sso.access_token(&store, id, Scope::ReadShipType).await
        && let Ok(ship) = esi.character_ship(&token, id).await
    {
        changed |= sqlx::query!(
            "update character_status
             set ship_type_id = $2, ship_name = $3, ship_item_id = $4,
                 ship_updated_at = case when ship_item_id is distinct from $4 then now()
                                        else ship_updated_at end,
                 updated_at = now()
             where character_id = $1",
            id,
            ship.ship_type_id,
            ship.ship_name,
            ship.ship_item_id,
        )
        .execute(&pool)
        .await
        .is_ok();
    }

    if changed {
        users.publish(
            due.user_id,
            UserEvent::CharacterStatusChanged { character_id: id },
        );
    }
}
