//! Discord alerts: rules a map watches for, and the messages they send.
//!
//! An alert is a standing question about the chain — "tell me when anything dies within
//! five jumps of us", "tell me when Jita comes within three" — that answers itself into a
//! Discord channel. The map is already the thing everyone has open; this is for when it
//! is not.
//!
//! Everything here is best-effort and off the critical path. Evaluating an alert must
//! never hold up ingesting a killmail or placing a system, and a Discord outage must cost
//! a message rather than a map change.

pub mod delivery;
pub mod filters;
pub mod killmail;
pub mod place;
pub mod proximity;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// The shared state alert evaluation needs: the stargate graph and an HTTP client.
///
/// Held for the life of the process. The graph is static reference data and the client
/// pools connections, so building either per killmail would cost more than the evaluation.
pub struct Runtime {
    universe: proximity::Universe,
    http: reqwest::Client,
}

impl Runtime {
    pub async fn load(pool: &PgPool) -> sqlx::Result<Runtime> {
        Ok(Runtime {
            universe: proximity::Universe::load(pool).await?,
            http: reqwest::Client::builder()
                .user_agent(concat!(
                    "vector-wormhole-mapper/",
                    env!("CARGO_PKG_VERSION"),
                    " (tim.kunze4@gmail.com)"
                ))
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        })
    }

    /// Offer a kill to every alert watching for one.
    pub async fn killmail(&self, pool: &PgPool, kill: &killmail::Kill) {
        killmail::evaluate(pool, &self.http, &self.universe, kill).await;
    }

    /// Re-evaluate a map's proximity alerts after its shape changed.
    pub async fn placed(&self, pool: &PgPool, map_id: i64, map_solar_system_id: i64) {
        place::evaluate(
            pool,
            &self.http,
            &self.universe,
            map_id,
            map_solar_system_id,
        )
        .await;
    }
}

/// Watch every map for the changes alerts care about.
///
/// A subscriber rather than a hook in the command dispatcher: alerts are an audience for
/// map changes, not a participant in them, and nothing here should be able to fail a
/// placement. Dropping events when it falls behind is the same trade.
pub fn start(pool: PgPool, hub: crate::maps::MapHub, runtime: std::sync::Arc<Runtime>) {
    tokio::spawn(async move {
        let mut events = hub.subscribe_all();
        loop {
            match events.recv().await {
                Ok(crate::maps::MapEvent::SystemAdded {
                    map_id,
                    map_solar_system_id,
                }) => {
                    runtime.placed(&pool, map_id, map_solar_system_id).await;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });
}

/// What an alert watches for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AlertKind {
    /// Something died within reach of the chain.
    Killmail,
    /// A system came within reach of the chain.
    Proximity,
    /// A system came within capital jump range of a k-space exit.
    JumpRange,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertKind::Killmail => "killmail",
            AlertKind::Proximity => "proximity",
            AlertKind::JumpRange => "jump_range",
        }
    }

    pub fn parse(value: &str) -> Option<AlertKind> {
        match value {
            "killmail" => Some(AlertKind::Killmail),
            "proximity" => Some(AlertKind::Proximity),
            "jump_range" => Some(AlertKind::JumpRange),
            _ => None,
        }
    }
}

/// Where the message goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AlertDelivery {
    /// A channel webhook URL, which needs no bot and no permissions.
    Webhook,
    /// A direct message to whoever created the alert.
    DiscordDm,
    /// A channel the bot can post in.
    DiscordChannel,
}

impl AlertDelivery {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertDelivery::Webhook => "webhook",
            AlertDelivery::DiscordDm => "discord_dm",
            AlertDelivery::DiscordChannel => "discord_channel",
        }
    }

    pub fn parse(value: &str) -> Option<AlertDelivery> {
        match value {
            "webhook" => Some(AlertDelivery::Webhook),
            "discord_dm" => Some(AlertDelivery::DiscordDm),
            "discord_channel" => Some(AlertDelivery::DiscordChannel),
            _ => None,
        }
    }

    /// Whether delivery needs the bot rather than a webhook URL.
    pub fn needs_bot(self) -> bool {
        matches!(
            self,
            AlertDelivery::DiscordDm | AlertDelivery::DiscordChannel
        )
    }
}

/// Who gets pinged when an alert fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AlertMention {
    None,
    /// Whoever created the alert, by their linked Discord account.
    Creator,
    /// The configured role.
    Role,
    Everyone,
}

impl AlertMention {
    pub fn as_str(self) -> &'static str {
        match self {
            AlertMention::None => "none",
            AlertMention::Creator => "creator",
            AlertMention::Role => "role",
            AlertMention::Everyone => "everyone",
        }
    }

    pub fn parse(value: &str) -> Option<AlertMention> {
        match value {
            "none" => Some(AlertMention::None),
            "creator" => Some(AlertMention::Creator),
            "role" => Some(AlertMention::Role),
            "everyone" => Some(AlertMention::Everyone),
            _ => None,
        }
    }
}

/// Why an alert stopped firing, so the settings page can say more than "off".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    Manual,
    DiscordUnlinked,
    AccessRevoked,
    DestinationGone,
    DeliveryFailed,
}

impl DisabledReason {
    pub fn as_str(self) -> &'static str {
        match self {
            DisabledReason::Manual => "manual",
            DisabledReason::DiscordUnlinked => "discord_unlinked",
            DisabledReason::AccessRevoked => "access_revoked",
            DisabledReason::DestinationGone => "destination_gone",
            DisabledReason::DeliveryFailed => "delivery_failed",
        }
    }
}

/// One alert, as everything here works with it.
#[derive(Debug, Clone)]
pub struct Alert {
    pub id: i64,
    pub map_id: i64,
    pub created_by_user_id: Option<i64>,
    pub name: String,
    pub kind: AlertKind,
    pub delivery: AlertDelivery,
    pub webhook_url: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_channel_id: Option<String>,
    pub discord_role_id: Option<String>,
    pub mention: AlertMention,
    pub target_solar_system_id: Option<i64>,
    pub max_jumps: i32,
    pub filters: Vec<filters::Rule>,
    pub filter_match: filters::Match,
    pub is_active: bool,
}

/// Every active alert of a kind, across every map.
///
/// Loaded whole rather than per map: a killmail arrives without knowing which maps care,
/// and there are tens of alerts in total, not thousands.
pub async fn active(pool: &PgPool, kind: AlertKind) -> sqlx::Result<Vec<Alert>> {
    let rows = sqlx::query!(
        r#"select id, map_id, created_by_user_id, name, kind, delivery, webhook_url,
                  discord_guild_id, discord_channel_id, discord_role_id, mention,
                  target_solar_system_id, max_jumps, filters, filter_match, is_active
           from map_alerts
           where kind = $1 and is_active
           order by id"#,
        kind.as_str(),
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| Alert {
            id: row.id,
            map_id: row.map_id,
            created_by_user_id: row.created_by_user_id,
            name: row.name,
            kind: AlertKind::parse(&row.kind).unwrap_or(kind),
            delivery: AlertDelivery::parse(&row.delivery).unwrap_or(AlertDelivery::Webhook),
            webhook_url: row.webhook_url,
            discord_guild_id: row.discord_guild_id,
            discord_channel_id: row.discord_channel_id,
            discord_role_id: row.discord_role_id,
            mention: AlertMention::parse(&row.mention).unwrap_or(AlertMention::None),
            target_solar_system_id: row.target_solar_system_id,
            max_jumps: row.max_jumps,
            filters: serde_json::from_value(row.filters).unwrap_or_default(),
            filter_match: filters::Match::parse(&row.filter_match).unwrap_or(filters::Match::Any),
            is_active: row.is_active,
        })
        .collect())
}

/// Claim the right to deliver this alert for this occasion.
///
/// `false` means somebody already has: a retry of a send that half-finished, or a second
/// evaluation of the same killmail. The row is written before the message goes out, so a
/// crash between the two costs a message rather than sending it twice.
pub async fn claim(pool: &PgPool, alert_id: i64, dedup_key: &str) -> bool {
    sqlx::query!(
        "insert into map_alert_deliveries (map_alert_id, dedup_key)
         values ($1, $2) on conflict do nothing",
        alert_id,
        dedup_key,
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() == 1)
    .unwrap_or(false)
}

/// Mark a claimed delivery as actually sent, and stamp the alert.
pub async fn sent(pool: &PgPool, alert_id: i64, dedup_key: &str) {
    let _ = sqlx::query!(
        "update map_alert_deliveries set delivered_at = now()
         where map_alert_id = $1 and dedup_key = $2",
        alert_id,
        dedup_key,
    )
    .execute(pool)
    .await;
    let _ = sqlx::query!(
        "update map_alerts set last_fired_at = now() where id = $1",
        alert_id,
    )
    .execute(pool)
    .await;
}

/// Give up on a claimed delivery, so the next occasion can try again.
pub async fn unclaim(pool: &PgPool, alert_id: i64, dedup_key: &str) {
    let _ = sqlx::query!(
        "delete from map_alert_deliveries
         where map_alert_id = $1 and dedup_key = $2 and delivered_at is null",
        alert_id,
        dedup_key,
    )
    .execute(pool)
    .await;
}

/// Record something that happened to an alert.
pub async fn log(
    pool: &PgPool,
    alert_id: Option<i64>,
    map_id: i64,
    actor: Option<i64>,
    kind: &str,
    detail: Option<&str>,
) {
    let _ = sqlx::query!(
        "insert into map_alert_events (map_alert_id, map_id, actor_user_id, kind, detail)
         values ($1, $2, $3, $4, $5)",
        alert_id,
        map_id,
        actor,
        kind,
        detail,
    )
    .execute(pool)
    .await;
}

/// Turn an alert off, saying why.
pub async fn disable(pool: &PgPool, alert: &Alert, reason: DisabledReason, detail: Option<&str>) {
    let _ = sqlx::query!(
        "update map_alerts
         set is_active = false, disabled_at = now(), disabled_reason = $2, updated_at = now()
         where id = $1",
        alert.id,
        reason.as_str(),
    )
    .execute(pool)
    .await;
    log(
        pool,
        Some(alert.id),
        alert.map_id,
        None,
        "disabled",
        detail.or(Some(reason.as_str())),
    )
    .await;
}
