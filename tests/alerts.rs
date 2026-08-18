//! Alert plumbing: the delivery ledger that stops a message being sent twice, and the
//! filter matching that decides whether a kill is worth a message at all.

mod common;

use common::world;
use sqlx::PgPool;
use vector::alerts::filters::{Candidates, Match, Mode, Rule, Side, Subject};
use vector::alerts::{self, AlertKind};

async fn make_alert(pool: &PgPool, map_id: i64, kind: &str) -> i64 {
    let webhook = sqlx::query_scalar!(
        "insert into map_webhooks (map_id, name, url)
         values ($1, 'Test channel', 'https://discord.com/api/webhooks/1/x')
         on conflict (map_id, name) do update set url = excluded.url
         returning id",
        map_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar!(
        "insert into map_alerts (map_id, name, kind, delivery, map_webhook_id, max_jumps)
         values ($1, 'Test', $2, 'webhook', $3, 5)
         returning id",
        map_id,
        kind,
        webhook,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The claim is the whole reason a retry cannot double-post.
#[sqlx::test]
async fn an_occasion_can_only_be_claimed_once(pool: PgPool) {
    let w = world(&pool).await;
    let alert = make_alert(&pool, w.map_id, "killmail").await;

    assert!(alerts::claim(&pool, alert, "killmail:1").await);
    assert!(!alerts::claim(&pool, alert, "killmail:1").await);
    // A different occasion is free to claim.
    assert!(alerts::claim(&pool, alert, "killmail:2").await);
}

/// Giving up must put the occasion back, or one Discord hiccup silences that kill forever.
#[sqlx::test]
async fn releasing_an_unsent_claim_allows_another_attempt(pool: PgPool) {
    let w = world(&pool).await;
    let alert = make_alert(&pool, w.map_id, "killmail").await;

    assert!(alerts::claim(&pool, alert, "killmail:1").await);
    alerts::unclaim(&pool, alert, "killmail:1").await;
    assert!(alerts::claim(&pool, alert, "killmail:1").await);
}

/// A delivered one stays claimed, whatever else happens.
#[sqlx::test]
async fn a_sent_claim_is_never_released(pool: PgPool) {
    let w = world(&pool).await;
    let alert = make_alert(&pool, w.map_id, "killmail").await;

    alerts::claim(&pool, alert, "killmail:1").await;
    alerts::sent(&pool, alert, "killmail:1").await;
    alerts::unclaim(&pool, alert, "killmail:1").await;
    assert!(!alerts::claim(&pool, alert, "killmail:1").await);

    let fired: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar!("select last_fired_at from map_alerts where id = $1", alert)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(fired.is_some());
}

#[sqlx::test]
async fn only_active_alerts_of_the_asked_kind_are_loaded(pool: PgPool) {
    let w = world(&pool).await;
    let killmail = make_alert(&pool, w.map_id, "killmail").await;
    let proximity = make_alert(&pool, w.map_id, "proximity").await;
    sqlx::query!(
        "update map_alerts set is_active = false where id = $1",
        proximity,
    )
    .execute(&pool)
    .await
    .unwrap();

    let loaded = alerts::active(&pool, AlertKind::Killmail).await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, killmail);
    assert!(
        alerts::active(&pool, AlertKind::Proximity)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Disabling records why, so the settings page can say more than "off".
#[sqlx::test]
async fn disabling_records_a_reason_and_an_event(pool: PgPool) {
    let w = world(&pool).await;
    let id = make_alert(&pool, w.map_id, "killmail").await;
    let alert = alerts::active(&pool, AlertKind::Killmail)
        .await
        .unwrap()
        .remove(0);

    alerts::disable(&pool, &alert, alerts::DisabledReason::DestinationGone, None).await;

    let row = sqlx::query!(
        "select is_active, disabled_reason from map_alerts where id = $1",
        id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!row.is_active);
    assert_eq!(row.disabled_reason.as_deref(), Some("destination_gone"));

    let events = sqlx::query_scalar!(
        "select kind from map_alert_events where map_alert_id = $1",
        id,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(events, vec!["disabled".to_string()]);
}

/// The filter vocabulary, end to end against a realistic kill.
#[test]
fn filters_decide_what_is_worth_a_message() {
    let kill = Candidates {
        victim_alliance: Some(99000001),
        victim_ship_type: Some(29990),
        attacker_alliance: Some(99000002),
        ..Default::default()
    };
    let watch_them = Rule {
        subject: Subject::Alliance,
        side: Side::Either,
        mode: Mode::Include,
        ids: vec![99000002],
    };
    assert!(vector::alerts::filters::matches(
        std::slice::from_ref(&watch_them),
        Match::Any,
        &kill
    ));

    // "Anything involving them, except when it is us dying" is the shape people actually
    // want, and it only works if an exclusion outranks a match.
    let not_our_losses = Rule {
        subject: Subject::Alliance,
        side: Side::Victim,
        mode: Mode::Exclude,
        ids: vec![99000001],
    };
    assert!(!vector::alerts::filters::matches(
        &[watch_them, not_our_losses],
        Match::Any,
        &kill
    ));
}
