//! Kills on the systems of one map.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum_extra::extract::CookieJar;

use super::ApiResult;
use super::extract::{ShareQuery, read_map_as};
use crate::auth::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/maps/{id}/killmails", get(map_killmails))
}

/// `GET /api/maps/{id}/killmails`, recent kills in this map's systems, newest first.
/// Viewer+, like reading the graph: a killmail is public record on zKillboard anyway.
pub async fn map_killmails(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<crate::killmails::MapKillmail>> {
    let reader = read_map_as(&state, &jar, map_id, &share).await?;
    // Which kills to show is a per-user preference, and a watcher has nowhere to keep one.
    let filter = match reader.actor {
        Some(actor) => sqlx::query_scalar!(
            "select killmail_filter from map_user_settings where map_id = $1 and user_id = $2",
            map_id,
            actor.user_id,
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(crate::maps::KillmailScope::All),
        None => crate::maps::KillmailScope::All,
    };

    Ok(Json(
        crate::killmails::list_for_map(
            &state.db,
            map_id,
            filter.into(),
            crate::killmails::CARD_LIMIT,
        )
        .await?,
    ))
}
