//! The Discord half: account linking, and what unlinking takes with it.

mod common;

use sqlx::PgPool;
use wormholesystems::discord;

async fn link(pool: &PgPool, user_id: i64, discord_id: &str) {
    sqlx::query!(
        "insert into discord_accounts (user_id, discord_user_id, username)
         values ($1, $2, 'pilot')",
        user_id,
        discord_id,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn an_account_is_found_from_either_side(pool: PgPool) {
    let w = common::world(&pool).await;
    link(&pool, w.owner.user_id, "9001").await;

    let account = discord::account_for(&pool, w.owner.user_id).await.unwrap();
    assert_eq!(account.discord_user_id, "9001");
    assert_eq!(
        discord::user_for(&pool, "9001").await,
        Some(w.owner.user_id)
    );
    assert_eq!(discord::user_for(&pool, "nobody").await, None);
}

/// A direct message with nobody to send it to cannot work, so unlinking turns those alerts
/// off with a reason rather than leaving them to fail forever.
#[sqlx::test]
async fn unlinking_stops_the_alerts_that_needed_it(pool: PgPool) {
    let w = common::world(&pool).await;
    link(&pool, w.owner.user_id, "9001").await;

    let dm = sqlx::query_scalar!(
        "insert into map_alerts (map_id, created_by_user_id, name, kind, delivery, max_jumps)
         values ($1, $2, 'DM me', 'killmail', 'discord_dm', 5) returning id",
        w.map_id,
        w.owner.user_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // A webhook alert needs no Discord account, so it must survive.
    let destination = sqlx::query_scalar!(
        "insert into map_webhooks (map_id, name, url)
         values ($1, 'Channel', 'https://discord.com/api/webhooks/1/x') returning id",
        w.map_id,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let webhook = sqlx::query_scalar!(
        "insert into map_alerts (map_id, created_by_user_id, name, kind, delivery, map_webhook_id, max_jumps)
         values ($1, $2, 'Channel', 'killmail', 'webhook', $3, 5)
         returning id",
        w.map_id,
        w.owner.user_id,
        destination,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    discord::link::unlink(&pool, w.owner.user_id).await;

    assert!(discord::account_for(&pool, w.owner.user_id).await.is_none());
    let dm_row = sqlx::query!(
        "select is_active, disabled_reason from map_alerts where id = $1",
        dm,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!dm_row.is_active);
    assert_eq!(dm_row.disabled_reason.as_deref(), Some("discord_unlinked"));

    let webhook_active =
        sqlx::query_scalar!("select is_active from map_alerts where id = $1", webhook,)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(webhook_active);

    // And it is on the record, so the settings page can explain itself.
    let events = sqlx::query_scalar!(
        "select detail from map_alert_events where map_alert_id = $1",
        dm,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(events, vec![Some("discord_unlinked".to_string())]);
}
