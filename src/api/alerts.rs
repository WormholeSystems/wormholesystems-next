//! The alerts API: what a map watches for, and what happened to those watches.
//!
//! Manager+ throughout. An alert carries a webhook URL or a channel id, which is a key to
//! somebody's Discord server; the people who can hand out map access are the people who
//! should be able to point the map at a channel.

use axum::extract::{Path, State};
use axum::{Json, extract::Query};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::alerts::{AlertDelivery, AlertKind, AlertMention, filters};
use crate::auth::AppState;
use crate::maps::{MapError, Role};

use super::{ApiError, ApiResult, require_actor};

/// One alert as the settings page shows it.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapAlert {
    pub id: i64,
    pub map_id: i64,
    pub name: String,
    pub kind: AlertKind,
    pub delivery: AlertDelivery,
    /// Never the URL itself: it is a bearer token for someone's channel, and an alert
    /// list is read by everyone with Manager, not just whoever pasted it in.
    #[ts(optional)]
    pub webhook_host: Option<String>,
    #[ts(optional)]
    pub discord_channel_id: Option<String>,
    #[ts(optional)]
    pub discord_role_id: Option<String>,
    pub mention: AlertMention,
    #[ts(optional)]
    pub target_solar_system_id: Option<i64>,
    #[ts(optional)]
    pub target_system_name: Option<String>,
    pub max_jumps: i32,
    pub filters: Vec<filters::Rule>,
    pub filter_match: filters::Match,
    pub is_active: bool,
    #[ts(optional)]
    pub disabled_reason: Option<String>,
    #[ts(optional)]
    pub last_fired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// One line of an alert's history.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapAlertEvent {
    pub id: i64,
    #[ts(optional)]
    pub map_alert_id: Option<i64>,
    #[ts(optional)]
    pub alert_name: Option<String>,
    #[ts(optional)]
    pub actor: Option<String>,
    pub kind: String,
    #[ts(optional)]
    pub detail: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn require_manager(state: &AppState, jar: &CookieJar, map_id: i64) -> Result<i64, ApiError> {
    let actor = require_actor(&state.db, jar).await?;
    match crate::maps::access::effective_role(&state.db, map_id, actor.user_id).await? {
        None => Err(ApiError::from(MapError::NotFound)),
        Some(role) if role >= Role::Manager => Ok(actor.user_id),
        Some(_) => Err(ApiError::from(MapError::Forbidden)),
    }
}

/// `GET /api/maps/{id}/alerts` — every alert on the map. Manager+.
pub async fn list_alerts(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<MapAlert>> {
    require_manager(&state, &jar, map_id).await?;
    Ok(Json(load(&state.db, map_id).await?))
}

async fn load(pool: &PgPool, map_id: i64) -> Result<Vec<MapAlert>, ApiError> {
    let rows = sqlx::query!(
        r#"select a.id, a.map_id, a.name, a.kind, a.delivery, a.webhook_url,
                  a.discord_channel_id, a.discord_role_id, a.mention,
                  a.target_solar_system_id, ss.name as "target_system_name?",
                  a.max_jumps, a.filters, a.filter_match, a.is_active,
                  a.disabled_reason, a.last_fired_at, a.created_at
           from map_alerts a
           left join solar_systems ss on ss.id = a.target_solar_system_id
           where a.map_id = $1
           order by a.id"#,
        map_id,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| MapAlert {
            id: row.id,
            map_id: row.map_id,
            name: row.name,
            kind: AlertKind::parse(&row.kind).unwrap_or(AlertKind::Killmail),
            delivery: AlertDelivery::parse(&row.delivery).unwrap_or(AlertDelivery::Webhook),
            webhook_host: row.webhook_url.as_deref().map(webhook_host),
            discord_channel_id: row.discord_channel_id,
            discord_role_id: row.discord_role_id,
            mention: AlertMention::parse(&row.mention).unwrap_or(AlertMention::None),
            target_solar_system_id: row.target_solar_system_id,
            target_system_name: row.target_system_name,
            max_jumps: row.max_jumps,
            filters: serde_json::from_value(row.filters).unwrap_or_default(),
            filter_match: filters::Match::parse(&row.filter_match).unwrap_or(filters::Match::Any),
            is_active: row.is_active,
            disabled_reason: row.disabled_reason,
            last_fired_at: row.last_fired_at,
            created_at: row.created_at,
        })
        .collect())
}

/// Enough of the URL to tell two destinations apart, without handing over either.
fn webhook_host(url: &str) -> String {
    let rest = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = rest.split('/').next().unwrap_or(rest);
    // The id is public-ish and stable; the token after it is the secret.
    let id = rest
        .split('/')
        .nth(3)
        .filter(|part| part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty());
    match id {
        Some(id) => format!("{host}/…/{id}"),
        None => host.to_string(),
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SaveAlert {
    pub name: String,
    pub kind: AlertKind,
    pub delivery: AlertDelivery,
    /// Write-only. Absent on an update leaves the stored one alone.
    #[serde(default)]
    #[ts(optional)]
    pub webhook_url: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub discord_guild_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub discord_channel_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub discord_role_id: Option<String>,
    pub mention: AlertMention,
    #[serde(default)]
    #[ts(optional)]
    pub target_solar_system_id: Option<i64>,
    pub max_jumps: i32,
    #[serde(default)]
    pub filters: Vec<filters::Rule>,
    pub filter_match: filters::Match,
}

impl SaveAlert {
    fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty() {
            return Err(ApiError::bad_request("an alert needs a name"));
        }
        if !(0..=30).contains(&self.max_jumps) {
            return Err(ApiError::bad_request("jumps must be between 0 and 30"));
        }
        match self.delivery {
            AlertDelivery::Webhook => {
                let url = self.webhook_url.as_deref().unwrap_or("");
                if !url.is_empty() && !url.starts_with("https://discord.com/api/webhooks/") {
                    return Err(ApiError::bad_request("that is not a Discord webhook URL"));
                }
            }
            AlertDelivery::DiscordChannel => {
                if self.discord_channel_id.is_none() {
                    return Err(ApiError::bad_request("pick a channel to post in"));
                }
            }
            AlertDelivery::DiscordDm => {}
        }
        if self.mention == AlertMention::Role && self.discord_role_id.is_none() {
            return Err(ApiError::bad_request("pick a role to mention"));
        }
        // Proximity and jump range are about a place; a killmail alert is about who.
        if matches!(self.kind, AlertKind::Proximity | AlertKind::JumpRange)
            && self.target_solar_system_id.is_none()
        {
            return Err(ApiError::bad_request("pick a system to watch"));
        }
        Ok(())
    }
}

/// `POST /api/maps/{id}/alerts` — create one. Manager+.
pub async fn create_alert(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(body): Json<SaveAlert>,
) -> ApiResult<MapAlert> {
    let user_id = require_manager(&state, &jar, map_id).await?;
    body.validate()?;
    if body.delivery == AlertDelivery::Webhook && body.webhook_url.is_none() {
        return Err(ApiError::bad_request("paste the channel's webhook URL"));
    }
    let id = sqlx::query_scalar!(
        "insert into map_alerts
             (map_id, created_by_user_id, name, kind, delivery, webhook_url,
              discord_guild_id, discord_channel_id, discord_role_id, mention,
              target_solar_system_id, max_jumps, filters, filter_match)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         returning id",
        map_id,
        user_id,
        body.name.trim(),
        body.kind.as_str(),
        body.delivery.as_str(),
        body.webhook_url,
        body.discord_guild_id,
        body.discord_channel_id,
        body.discord_role_id,
        body.mention.as_str(),
        body.target_solar_system_id,
        body.max_jumps,
        serde_json::to_value(&body.filters).unwrap_or_else(|_| serde_json::json!([])),
        body.filter_match.as_str(),
    )
    .fetch_one(&state.db)
    .await?;
    crate::alerts::log(&state.db, Some(id), map_id, Some(user_id), "created", None).await;
    one(&state.db, map_id, id).await
}

/// `PUT /api/maps/{id}/alerts/{alert_id}` — replace its settings. Manager+.
pub async fn update_alert(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, alert_id)): Path<(i64, i64)>,
    Json(body): Json<SaveAlert>,
) -> ApiResult<MapAlert> {
    let user_id = require_manager(&state, &jar, map_id).await?;
    body.validate()?;
    let updated = sqlx::query!(
        "update map_alerts set
             name = $3, kind = $4, delivery = $5,
             webhook_url = coalesce($6, webhook_url),
             discord_guild_id = $7, discord_channel_id = $8, discord_role_id = $9,
             mention = $10, target_solar_system_id = $11, max_jumps = $12,
             filters = $13, filter_match = $14, updated_at = now()
         where id = $1 and map_id = $2",
        alert_id,
        map_id,
        body.name.trim(),
        body.kind.as_str(),
        body.delivery.as_str(),
        body.webhook_url,
        body.discord_guild_id,
        body.discord_channel_id,
        body.discord_role_id,
        body.mention.as_str(),
        body.target_solar_system_id,
        body.max_jumps,
        serde_json::to_value(&body.filters).unwrap_or_else(|_| serde_json::json!([])),
        body.filter_match.as_str(),
    )
    .execute(&state.db)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::from(MapError::NotFound));
    }
    crate::alerts::log(
        &state.db,
        Some(alert_id),
        map_id,
        Some(user_id),
        "updated",
        None,
    )
    .await;
    one(&state.db, map_id, alert_id).await
}

#[derive(Debug, Clone, Deserialize, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SetAlertActive {
    pub is_active: bool,
}

/// `POST /api/maps/{id}/alerts/{alert_id}/active` — turn it on or off by hand. Manager+.
pub async fn set_alert_active(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, alert_id)): Path<(i64, i64)>,
    Json(body): Json<SetAlertActive>,
) -> ApiResult<MapAlert> {
    let user_id = require_manager(&state, &jar, map_id).await?;
    // Turning it back on clears the reason it stopped: whatever went wrong, somebody has
    // looked at it and says it is fixed.
    let updated = sqlx::query!(
        "update map_alerts set
             is_active = $3,
             disabled_at = case when $3 then null else now() end,
             disabled_reason = case when $3 then null else 'manual' end,
             updated_at = now()
         where id = $1 and map_id = $2",
        alert_id,
        map_id,
        body.is_active,
    )
    .execute(&state.db)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::from(MapError::NotFound));
    }
    crate::alerts::log(
        &state.db,
        Some(alert_id),
        map_id,
        Some(user_id),
        if body.is_active {
            "enabled"
        } else {
            "disabled"
        },
        None,
    )
    .await;
    one(&state.db, map_id, alert_id).await
}

/// `DELETE /api/maps/{id}/alerts/{alert_id}`. Manager+.
pub async fn delete_alert(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((map_id, alert_id)): Path<(i64, i64)>,
) -> ApiResult<()> {
    let user_id = require_manager(&state, &jar, map_id).await?;
    let name = sqlx::query_scalar!(
        "delete from map_alerts where id = $1 and map_id = $2 returning name",
        alert_id,
        map_id,
    )
    .fetch_optional(&state.db)
    .await?;
    let Some(name) = name else {
        return Err(ApiError::from(MapError::NotFound));
    };
    crate::alerts::log(
        &state.db,
        None,
        map_id,
        Some(user_id),
        "deleted",
        Some(&name),
    )
    .await;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    #[serde(default)]
    pub limit: Option<i64>,
}

/// `GET /api/maps/{id}/alerts/events` — the audit trail. Manager+.
pub async fn list_alert_events(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(query): Query<EventsQuery>,
) -> ApiResult<Vec<MapAlertEvent>> {
    require_manager(&state, &jar, map_id).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query!(
        r#"select e.id, e.map_alert_id, a.name as "alert_name?", e.kind, e.detail, e.created_at,
                  (select c.name from characters c
                    where c.user_id = e.actor_user_id
                    order by c.id limit 1) as "actor?"
           from map_alert_events e
           left join map_alerts a on a.id = e.map_alert_id
           where e.map_id = $1
           order by e.id desc
           limit $2"#,
        map_id,
        limit,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| MapAlertEvent {
                id: row.id,
                map_alert_id: row.map_alert_id,
                alert_name: row.alert_name,
                actor: row.actor,
                kind: row.kind,
                detail: row.detail,
                created_at: row.created_at,
            })
            .collect(),
    ))
}

async fn one(pool: &PgPool, map_id: i64, alert_id: i64) -> ApiResult<MapAlert> {
    load(pool, map_id)
        .await?
        .into_iter()
        .find(|a| a.id == alert_id)
        .map(Json)
        .ok_or_else(|| ApiError::from(MapError::NotFound))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_webhook_is_summarised_without_its_token() {
        let url = "https://discord.com/api/webhooks/123456/verysecrettoken";
        let shown = webhook_host(url);
        assert!(shown.contains("discord.com"));
        assert!(shown.contains("123456"));
        assert!(!shown.contains("verysecrettoken"));
    }
}
