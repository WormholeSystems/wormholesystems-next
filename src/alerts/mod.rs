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
pub mod jump_range;
pub mod killmail;
pub mod place;
pub mod proximity;
pub mod ships;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// The shared state alert evaluation needs: the stargate graph and an HTTP client.
///
/// Held for the life of the process. The graph is static reference data and the client
/// pools connections, so building either per killmail would cost more than the evaluation.
pub struct Runtime {
    universe: proximity::Universe,
    http: reqwest::Client,
    /// Absent unless a bot is configured; only channel and direct-message delivery need it.
    bot_token: Option<String>,
}

impl Runtime {
    pub async fn load(pool: &PgPool, bot_token: Option<String>) -> sqlx::Result<Runtime> {
        Ok(Runtime {
            bot_token,
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

    fn token(&self) -> Option<&str> {
        self.bot_token.as_deref()
    }

    /// Offer a kill to every alert watching for one.
    pub async fn killmail(&self, pool: &PgPool, kill: &killmail::Kill) {
        killmail::evaluate(pool, &self.http, self.token(), &self.universe, kill).await;
    }

    /// Re-evaluate a map's alerts after its shape changed.
    ///
    /// Both kinds that watch the map fire from here: a new system can put a target within
    /// gate range, within jump range, or neither.
    pub async fn placed(&self, pool: &PgPool, map_id: i64, map_solar_system_id: i64) {
        place::evaluate(
            pool,
            &self.http,
            self.token(),
            &self.universe,
            map_id,
            map_solar_system_id,
        )
        .await;
        jump_range::evaluate(pool, &self.http, self.token(), map_id, map_solar_system_id).await;
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
    /// Resolved from the named destination the alert points at.
    pub webhook_url: Option<String>,
    pub discord_guild_id: Option<String>,
    pub discord_channel_id: Option<String>,
    /// Resolved from the named role the alert points at.
    pub discord_role_id: Option<String>,
    pub mention: AlertMention,
    pub target_solar_system_id: Option<i64>,
    pub max_jumps: i32,
    /// Jump range only: which hull's range is being measured, and the pilot's JDC level.
    pub ship_type: Option<ships::JumpShip>,
    pub jdc_level: Option<i32>,
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
        r#"select a.id, a.map_id, a.created_by_user_id, a.name, a.kind, a.delivery,
                  w.url as "webhook_url?",
                  a.discord_guild_id, a.discord_channel_id,
                  r.discord_role_id as "discord_role_id?",
                  a.mention, a.target_solar_system_id, a.max_jumps, a.ship_type, a.jdc_level,
                  a.filters, a.filter_match, a.is_active
           from map_alerts a
           left join map_webhooks w on w.id = a.map_webhook_id
           left join map_webhook_roles r on r.id = a.map_webhook_role_id
           where a.kind = $1 and a.is_active
           order by a.id"#,
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
            ship_type: row.ship_type.as_deref().and_then(ships::JumpShip::parse),
            jdc_level: row.jdc_level,
            filters: serde_json::from_value(row.filters).unwrap_or_default(),
            filter_match: filters::Match::parse(&row.filter_match).unwrap_or(filters::Match::Any),
            is_active: row.is_active,
        })
        .collect())
}

/// Send one alert's message, wherever it is meant to go.
///
/// `Err(true)` means the destination is gone and the alert should stop; `Err(false)` means
/// try again next time. The three delivery types differ only in where the message is
/// addressed, so the mention, the retry and the rate limiting are decided once here.
pub async fn deliver(
    pool: &PgPool,
    http: &reqwest::Client,
    bot_token: Option<&str>,
    alert: &Alert,
    embed: delivery::Embed,
) -> Result<(), bool> {
    // Checked at send time rather than hooked to every access change: access can be lost in
    // half a dozen ways (a role revoked, a character moved corp, a whole grant deleted),
    // and this is the one place that must be right.
    if let Some(creator) = alert.created_by_user_id
        && !can_still_see(pool, alert.map_id, creator).await
    {
        disable(pool, alert, DisabledReason::AccessRevoked, None).await;
        return Err(false);
    }

    let creator_discord = match alert.created_by_user_id {
        Some(user_id) => crate::discord::account_for(pool, user_id)
            .await
            .map(|a| a.discord_user_id),
        None => None,
    };

    let message = match alert.mention {
        AlertMention::Role => match alert.discord_role_id.as_deref() {
            Some(role) => delivery::Message::new(embed).mention_role(role),
            None => delivery::Message::new(embed),
        },
        AlertMention::Everyone => delivery::Message::new(embed).mention_everyone(),
        AlertMention::Creator => match creator_discord.as_deref() {
            Some(user) => delivery::Message::new(embed).mention_user(user),
            // Asking to ping someone who has not linked is not a broken destination, just
            // a message without a ping.
            None => delivery::Message::new(embed),
        },
        AlertMention::None => delivery::Message::new(embed),
    };

    let result = match alert.delivery {
        AlertDelivery::Webhook => match alert.webhook_url.as_deref() {
            Some(url) => delivery::post_webhook(http, url, &message).await,
            None => return Err(true),
        },
        AlertDelivery::DiscordChannel => {
            match (bot_token, alert.discord_channel_id.as_deref()) {
                (Some(token), Some(channel)) => {
                    delivery::post_channel(http, token, channel, &message).await
                }
                // No bot configured is an operator problem, not a broken alert: leave it
                // active so it starts working when the token appears.
                (None, _) => return Err(false),
                (_, None) => return Err(true),
            }
        }
        AlertDelivery::DiscordDm => match (bot_token, creator_discord.as_deref()) {
            (Some(token), Some(user)) => delivery::post_dm(http, token, user, &message).await,
            (None, _) => return Err(false),
            (_, None) => {
                disable(pool, alert, DisabledReason::DiscordUnlinked, None).await;
                return Err(false);
            }
        },
    };

    match result {
        Ok(()) => Ok(()),
        Err(delivery::SendError::Gone) => Err(true),
        Err(delivery::SendError::Failed(err)) => {
            log(
                pool,
                Some(alert.id),
                alert.map_id,
                None,
                "failed",
                Some(&err),
            )
            .await;
            Err(false)
        }
    }
}

/// Whether the alert's creator can still see the map it watches.
async fn can_still_see(pool: &PgPool, map_id: i64, user_id: i64) -> bool {
    crate::maps::access::effective_role(pool, map_id, user_id)
        .await
        .ok()
        .flatten()
        .is_some()
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
