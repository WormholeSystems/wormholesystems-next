//! The per-user settings kept against a map: what each viewer has chosen to see, where
//! they have put their panels, and the toggles that are theirs alone rather than the
//! map's.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use crate::maps::{KillmailScope, MapLayout, MassStatus, RoutePreference, TimeStatus};

use super::extract::require_actor;
use super::layout::PanelLayouts;
use super::{ApiError, ApiResult};
use crate::auth::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/api/maps/{id}/settings/user",
        get(map_user_settings).post(update_map_user_settings),
    )
}

/// A user's per-map preferences.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapUserSettings {
    pub tracking_allowed: bool,
    pub show_threat_level: bool,
    pub compact_signature_list: bool,
    pub show_statics_first: bool,
    pub route_preference: RoutePreference,
    /// 0-100, weight of the security preference (legacy `exp(0.15 * penalty)`).
    pub security_penalty: i32,
    /// Worst wormhole lifetime still routed through.
    pub route_allow_time_status: TimeStatus,
    /// Worst wormhole mass still routed through.
    pub route_allow_mass_status: MassStatus,
    pub route_use_evescout: bool,
    /// Ask which signature was jumped, rather than mapping the hole unlinked.
    pub prompt_for_signature: bool,
    /// Prefill the jump dialog's alias from the chain's naming scheme.
    pub suggest_alias: bool,
    /// Put the new connection's bookmark on the clipboard once the jump is mapped.
    pub copy_bookmark: bool,
    /// Which half of the chain the killmails card shows.
    pub killmail_filter: KillmailScope,
    pub is_archived: bool,
    /// Whether this user has been through the map's introduction.
    pub introduction_confirmed: bool,
    /// Panels this user hides on this map. Empty = the built-in set. A hidden panel keeps
    /// its saved position, so unhiding puts it back where it was.
    pub hidden_panels: Vec<String>,
    /// This viewer's placement choice, when the map hands it to them. `None` follows the
    /// map's own mode.
    #[ts(optional)]
    pub layout_override: Option<MapLayout>,
    /// Whether this user keeps the map in the top bar for quick access.
    pub is_pinned: bool,
    /// Per-breakpoint tile positions. `None` = the built-in arrangement.
    #[ts(optional)]
    pub layout_breakpoints: Option<PanelLayouts>,
}

/// Partial update of [`MapUserSettings`]; absent fields stay unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateMapUserSettings {
    #[serde(default)]
    #[ts(optional)]
    pub is_pinned: Option<bool>,
    /// Absent leaves it; `null` goes back to following the map.
    #[serde(default, deserialize_with = "crate::maps::double_option")]
    #[ts(optional)]
    pub layout_override: Option<Option<MapLayout>>,
    #[serde(default)]
    #[ts(optional)]
    pub tracking_allowed: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub show_threat_level: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub compact_signature_list: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub show_statics_first: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub route_preference: Option<RoutePreference>,
    #[serde(default)]
    #[ts(optional)]
    pub security_penalty: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub route_allow_time_status: Option<TimeStatus>,
    #[serde(default)]
    #[ts(optional)]
    pub route_allow_mass_status: Option<MassStatus>,
    #[serde(default)]
    #[ts(optional)]
    pub route_use_evescout: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub prompt_for_signature: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub suggest_alias: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub copy_bookmark: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub killmail_filter: Option<KillmailScope>,
    #[serde(default)]
    #[ts(optional)]
    pub is_archived: Option<bool>,
    /// Stamped server-side, so "when" is the server's clock rather than the browser's.
    #[serde(default)]
    #[ts(optional)]
    pub introduction_confirmed: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub hidden_panels: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub layout_breakpoints: Option<PanelLayouts>,
}

/// `GET /api/maps/{id}/settings/user`: the caller's per-map preferences (defaults when
/// no row exists yet). Requires any access to the map.
pub async fn map_user_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<MapUserSettings> {
    let actor = require_actor(&state.db, &jar).await?;
    if crate::maps::access::effective_role(&state.db, map_id, actor.user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::from(crate::maps::MapError::NotFound));
    }
    let row = sqlx::query!(
        r#"select tracking_allowed, show_threat_level, compact_signature_list,
                  show_statics_first,
                  route_preference as "route_preference: RoutePreference", security_penalty,
                  route_allow_time_status as "route_allow_time_status: TimeStatus",
                  route_allow_mass_status as "route_allow_mass_status: MassStatus",
                  route_use_evescout,
                  prompt_for_signature, suggest_alias, copy_bookmark,
                  killmail_filter as "killmail_filter: KillmailScope",
                  is_archived,
                  (introduction_confirmed_at is not null) as "introduction_confirmed!",
                  hidden_panels, layout_breakpoints,
                  layout_override as "layout_override: MapLayout", is_pinned
           from map_user_settings where map_id = $1 and user_id = $2"#,
        map_id,
        actor.user_id,
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(Json(match row {
        Some(r) => MapUserSettings {
            tracking_allowed: r.tracking_allowed,
            show_threat_level: r.show_threat_level,
            compact_signature_list: r.compact_signature_list,
            show_statics_first: r.show_statics_first,
            route_preference: r.route_preference,
            security_penalty: r.security_penalty,
            route_allow_time_status: r.route_allow_time_status,
            route_allow_mass_status: r.route_allow_mass_status,
            route_use_evescout: r.route_use_evescout,
            prompt_for_signature: r.prompt_for_signature,
            suggest_alias: r.suggest_alias,
            copy_bookmark: r.copy_bookmark,
            killmail_filter: r.killmail_filter,
            is_archived: r.is_archived,
            introduction_confirmed: r.introduction_confirmed,
            hidden_panels: r.hidden_panels,
            layout_override: r.layout_override,
            is_pinned: r.is_pinned,
            layout_breakpoints: r
                .layout_breakpoints
                .map(serde_json::from_value)
                .transpose()
                .unwrap_or(None),
        },
        None => MapUserSettings {
            layout_override: None,
            is_pinned: false,
            tracking_allowed: false,
            show_threat_level: true,
            compact_signature_list: false,
            show_statics_first: false,
            route_preference: RoutePreference::Shorter,
            security_penalty: 50,
            route_allow_time_status: TimeStatus::Critical,
            route_allow_mass_status: MassStatus::Reduced,
            route_use_evescout: false,
            prompt_for_signature: true,
            suggest_alias: true,
            copy_bookmark: false,
            killmail_filter: KillmailScope::All,
            is_archived: false,
            introduction_confirmed: false,
            hidden_panels: Vec::new(),
            layout_breakpoints: None,
        },
    }))
}

/// `POST /api/maps/{id}/settings/user`, partial update (upsert) of the caller's per-map
/// preferences.
pub async fn update_map_user_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(body): Json<UpdateMapUserSettings>,
) -> ApiResult<MapUserSettings> {
    let actor = require_actor(&state.db, &jar).await?;
    if crate::maps::access::effective_role(&state.db, map_id, actor.user_id)
        .await?
        .is_none()
    {
        return Err(ApiError::from(crate::maps::MapError::NotFound));
    }
    // The tolerances and the preference are enums, so a value outside them never gets this
    // far: serde rejects the body. Only the number still needs saying out loud.
    if let Some(p) = body.security_penalty
        && !(0..=100).contains(&p)
    {
        return Err(ApiError::bad_request("security penalty must be 0-100"));
    }
    // Reject an arrangement that could not render, rather than breaking the next load.
    let layout_json =
        match &body.layout_breakpoints {
            Some(layouts) => {
                super::validate_layouts(layouts)?;
                Some(serde_json::to_value(layouts).map_err(|e| {
                    ApiError::bad_request(format!("could not store the layout: {e}"))
                })?)
            }
            None => None,
        };

    let row = sqlx::query!(
        r#"insert into map_user_settings
             (map_id, user_id, tracking_allowed, show_threat_level,
              compact_signature_list, show_statics_first,
              route_preference, security_penalty, route_allow_time_status,
              route_allow_mass_status, route_use_evescout, prompt_for_signature,
              suggest_alias, copy_bookmark, killmail_filter, is_archived,
              introduction_confirmed_at, hidden_panels, layout_breakpoints, layout_override,
              is_pinned)
         values ($1, $2, coalesce($3, false), coalesce($4, true),
                 coalesce($5, false), coalesce($6, false),
                 coalesce($7, 'shorter'), coalesce($8, 50), coalesce($9, 'critical'),
                 coalesce($10, 'reduced'), coalesce($11, false),
                 coalesce($12, true), coalesce($13, true), coalesce($14, false),
                 coalesce($15, 'all'), coalesce($16, false),
                 case when $17 then now() end,
                 coalesce($18, '{}'::text[]), $19, $20, coalesce($22, false))
         on conflict (map_id, user_id) do update set
             tracking_allowed = coalesce($3, map_user_settings.tracking_allowed),
             show_threat_level = coalesce($4, map_user_settings.show_threat_level),
             compact_signature_list = coalesce($5, map_user_settings.compact_signature_list),
             show_statics_first = coalesce($6, map_user_settings.show_statics_first),
             route_preference = coalesce($7, map_user_settings.route_preference),
             security_penalty = coalesce($8, map_user_settings.security_penalty),
             route_allow_time_status = coalesce($9, map_user_settings.route_allow_time_status),
             route_allow_mass_status = coalesce($10, map_user_settings.route_allow_mass_status),
             route_use_evescout = coalesce($11, map_user_settings.route_use_evescout),
             prompt_for_signature = coalesce($12, map_user_settings.prompt_for_signature),
             suggest_alias = coalesce($13, map_user_settings.suggest_alias),
             copy_bookmark = coalesce($14, map_user_settings.copy_bookmark),
             killmail_filter = coalesce($15, map_user_settings.killmail_filter),
             is_archived = coalesce($16, map_user_settings.is_archived),
             -- Absent leaves it; true stamps now; false clears it and shows it again.
             introduction_confirmed_at = case
                 when $17 is null then map_user_settings.introduction_confirmed_at
                 when $17 then now()
             end,
             hidden_panels = coalesce($18, map_user_settings.hidden_panels),
             layout_breakpoints = coalesce($19, map_user_settings.layout_breakpoints),
             -- Absent leaves it; null is a real value here (follow the map again), so it
             -- cannot go through coalesce.
             layout_override = case
                 when $21 then $20
                 else map_user_settings.layout_override
             end,
             is_pinned = coalesce($22, map_user_settings.is_pinned),
             updated_at = now()
         returning tracking_allowed, show_threat_level, compact_signature_list,
                   show_statics_first,
                   route_preference as "route_preference: RoutePreference", security_penalty,
                   route_allow_time_status as "route_allow_time_status: TimeStatus",
                   route_allow_mass_status as "route_allow_mass_status: MassStatus",
                   route_use_evescout,
                   prompt_for_signature, suggest_alias, copy_bookmark,
                   killmail_filter as "killmail_filter: KillmailScope",
                   is_archived,
                   (introduction_confirmed_at is not null) as introduction_confirmed,
                   hidden_panels, layout_breakpoints,
                   layout_override as "layout_override: MapLayout", is_pinned"#,
        map_id,
        actor.user_id,
        body.tracking_allowed,
        body.show_threat_level,
        body.compact_signature_list,
        body.show_statics_first,
        body.route_preference.map(|p| p.as_str()),
        body.security_penalty,
        body.route_allow_time_status.map(|t| t.as_str()),
        body.route_allow_mass_status.map(|m| m.as_str()),
        body.route_use_evescout,
        body.prompt_for_signature,
        body.suggest_alias,
        body.copy_bookmark,
        body.killmail_filter.map(|k| k.as_str()),
        body.is_archived,
        body.introduction_confirmed,
        body.hidden_panels.as_deref(),
        layout_json.as_ref(),
        body.layout_override.flatten().map(|l| l.as_str()),
        body.layout_override.is_some(),
        body.is_pinned,
    )
    .fetch_one(&state.db)
    .await?;

    // Whether this user shares their position decides whether they appear on everyone
    // else's pilot list, and the poller only announces maps that are already shared: with
    // nothing published here, switching it off never reaches the other viewers at all.
    if body.tracking_allowed.is_some() {
        state
            .hub
            .publish(crate::maps::MapEvent::CharactersChanged { map_id });
    }

    Ok(Json(MapUserSettings {
        layout_override: row.layout_override,
        is_pinned: row.is_pinned,
        tracking_allowed: row.tracking_allowed,
        show_threat_level: row.show_threat_level,
        compact_signature_list: row.compact_signature_list,
        show_statics_first: row.show_statics_first,
        route_preference: row.route_preference,
        security_penalty: row.security_penalty,
        route_allow_time_status: row.route_allow_time_status,
        route_allow_mass_status: row.route_allow_mass_status,
        route_use_evescout: row.route_use_evescout,
        prompt_for_signature: row.prompt_for_signature,
        suggest_alias: row.suggest_alias,
        copy_bookmark: row.copy_bookmark,
        killmail_filter: row.killmail_filter,
        is_archived: row.is_archived,
        introduction_confirmed: row.introduction_confirmed.unwrap_or(false),
        hidden_panels: row.hidden_panels,
        layout_breakpoints: row
            .layout_breakpoints
            .map(serde_json::from_value)
            .transpose()
            .unwrap_or(None),
    }))
}
