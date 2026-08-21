//! Two-tier character-status polling, see
//! [processes.md](../docs/processes.md#character-status-polling).
//!
//! Tier 1 polls online state for the characters of active users every 60s; tier 2 polls
//! location and ship for the online ones every 5s. Both are bounded by a [`Semaphore`] so we
//! stay within ESI's error limit and fit each tier's time budget. A poll that observes an
//! actual change pings its user's private channel so their UI refetches.

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
use crate::maps::MapHub;
use crate::server_status::ServerWatch;
use crate::user_channel::{UserEvent, UserHub};

/// Max ESI requests in flight per tick, the one tuning knob. Raise to fit more characters
/// in the 5s tier-2 budget; lower to stay further under ESI's error limit.
const CONCURRENCY: usize = 32;

/// A character due for polling, with the user to notify.
#[derive(Clone, Copy)]
struct Due {
    character_id: i64,
    user_id: i64,
}

/// Spawn the polling loops. Returns immediately; the loops run for the process lifetime.
/// `maps` receives `ConnectionChanged` events when a transit lands on a mapped hole.
pub fn start(
    pool: PgPool,
    sso: Arc<Sso>,
    esi: EsiClient,
    users: UserHub,
    maps: MapHub,
    server: ServerWatch,
) {
    tokio::spawn(tier_one(
        pool.clone(),
        sso.clone(),
        esi.clone(),
        users.clone(),
        maps.clone(),
        server.clone(),
    ));
    tokio::spawn(tier_two(pool, sso, esi, users, maps, server));
}

/// Tier 1: online state, every 60s, for every character of an active user.
async fn tier_one(
    pool: PgPool,
    sso: Arc<Sso>,
    esi: EsiClient,
    users: UserHub,
    maps: MapHub,
    server: ServerWatch,
) {
    let mut ticker = interval(Duration::from_secs(60));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        // Nobody can be online while the server is not, and every call would fail against
        // the error limit. See [`crate::server_status`].
        if !server.should_poll() {
            continue;
        }
        match active_characters(&pool, false).await {
            Ok(due) => {
                run_bounded(&due, CONCURRENCY, |d| {
                    poll_online(
                        pool.clone(),
                        sso.clone(),
                        esi.clone(),
                        users.clone(),
                        maps.clone(),
                        d,
                    )
                })
                .await
            }
            Err(err) => eprintln!("tracking tier-1 select failed: {err}"),
        }
    }
}

/// Tier 2: location + ship, every 5s, for currently-online characters of active users.
async fn tier_two(
    pool: PgPool,
    sso: Arc<Sso>,
    esi: EsiClient,
    users: UserHub,
    maps: MapHub,
    server: ServerWatch,
) {
    let mut ticker = interval(Duration::from_secs(5));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        if !server.should_poll() {
            continue;
        }
        match active_characters(&pool, true).await {
            Ok(due) => {
                run_bounded(&due, CONCURRENCY, |d| {
                    poll_location_ship(
                        pool.clone(),
                        sso.clone(),
                        esi.clone(),
                        users.clone(),
                        maps.clone(),
                        d,
                    )
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
            r#"select c.id, c.user_id as "user_id!"
             from characters c
             join users u on u.id = c.user_id
             join character_status s on s.character_id = c.id
             where u.last_active_at > now() - interval '5 minutes' and s.online"#
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
            r#"select c.id, c.user_id as "user_id!"
             from characters c
             join users u on u.id = c.user_id
             where u.last_active_at > now() - interval '5 minutes'"#
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
/// character; the next tick retries.
async fn poll_online(
    pool: PgPool,
    sso: Arc<Sso>,
    esi: EsiClient,
    users: UserHub,
    maps: MapHub,
    due: Due,
) {
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
    // The write always runs (`updated_at` is the last successful poll), but the user is only
    // pinged on an actual transition, so the CTE snapshots the prior value to compare against.
    let Ok(row) = sqlx::query!(
        r#"with prev as (
             select online from character_status where character_id = $1
         )
         insert into character_status (character_id, online, last_online_at)
         values ($1, $2, case when $2 then now() else null end)
         on conflict (character_id) do update set
             online = excluded.online,
             last_online_at = case when excluded.online then now()
                                   else character_status.last_online_at end,
             updated_at = now()
         returning (select online from prev) as "prev_online?""#,
        due.character_id,
        status.online,
    )
    .fetch_one(&pool)
    .await
    else {
        return;
    };
    if row.prev_online != Some(status.online) {
        users.publish(
            due.user_id,
            UserEvent::CharacterStatusChanged {
                character_id: due.character_id,
            },
        );
        announce_presence(&pool, &maps, due.user_id).await;
    }
}

/// Poll one character's location and ship (tier 2). The two use independent scopes, so a
/// missing one skips just that field. Docking is left null until we cache those entities.
async fn poll_location_ship(
    pool: PgPool,
    sso: Arc<Sso>,
    esi: EsiClient,
    users: UserHub,
    maps: MapHub,
    due: Due,
) {
    let store = PgTokenStore::new(pool.clone());
    let id = due.character_id;
    let mut changed = false;

    // As in tier 1: the writes always run (poll freshness), but `changed` compares against
    // the prior values so the user is only pinged when something actually changed.
    if let Ok(token) = sso.access_token(&store, id, Scope::ReadLocation).await
        && let Ok(location) = esi.character_location(&token, id).await
        && let Ok(Some(row)) = sqlx::query!(
            r#"with prev as (
                 select solar_system_id, station_id, structure_id
                 from character_status where character_id = $1
             )
             update character_status
             set solar_system_id = $2, station_id = null, structure_id = null, updated_at = now()
             where character_id = $1
             returning (select solar_system_id from prev) as "prev_solar_system_id?",
                       (select station_id from prev) as "prev_station_id?",
                       (select structure_id from prev) as "prev_structure_id?""#,
            id,
            location.solar_system_id,
        )
        .fetch_optional(&pool)
        .await
    {
        changed |= row.prev_solar_system_id != Some(location.solar_system_id)
            || row.prev_station_id.is_some()
            || row.prev_structure_id.is_some();

        // A system change is a potential wormhole transit: jump capture must never
        // break polling, so failures are logged and dropped.
        if let Some(prev) = row.prev_solar_system_id
            && prev != location.solar_system_id
            && let Err(err) =
                crate::maps::jumps::record_transit(&pool, &maps, id, prev, location.solar_system_id)
                    .await
        {
            eprintln!("jump capture failed for character {id}: {err}");
        }
    }

    if let Ok(token) = sso.access_token(&store, id, Scope::ReadShipType).await
        && let Ok(ship) = esi.character_ship(&token, id).await
        && let Ok(Some(row)) = sqlx::query!(
            // The stamp moves only when the hull does, so it answers "how long have they
            // been in this ship" rather than "when did we last poll".
            r#"with prev as (
                 select ship_type_id, ship_name from character_status where character_id = $1
             )
             update character_status
             set ship_type_id = $2, ship_name = $3, ship_item_id = $4,
                 ship_updated_at = case
                     when ship_item_id is distinct from $4 then now()
                     else ship_updated_at
                 end,
                 updated_at = now()
             where character_id = $1
             returning (select ship_type_id from prev) as "prev_ship_type_id?",
                       (select ship_name from prev) as "prev_ship_name?""#,
            id,
            ship.ship_type_id,
            ship.ship_name,
            ship.ship_item_id,
        )
        .fetch_optional(&pool)
        .await
    {
        changed |= row.prev_ship_type_id != Some(ship.ship_type_id)
            || row.prev_ship_name.as_deref() != Some(ship.ship_name.as_str());
    }

    if changed {
        users.publish(
            due.user_id,
            UserEvent::CharacterStatusChanged { character_id: id },
        );
        announce_presence(&pool, &maps, due.user_id).await;
    }
}

/// Tell every map this user shares their position with that its pilot list moved. The
/// per-user ping above only reaches the pilot's own tabs.
async fn announce_presence(pool: &PgPool, maps: &MapHub, user_id: i64) {
    let Ok(map_ids) = sqlx::query_scalar!(
        "select map_id from map_user_settings where user_id = $1 and tracking_allowed",
        user_id,
    )
    .fetch_all(pool)
    .await
    else {
        return;
    };
    for map_id in map_ids {
        maps.publish(crate::maps::MapEvent::CharactersChanged { map_id });
    }
}
