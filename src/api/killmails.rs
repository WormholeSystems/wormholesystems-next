//! Kills on the systems of one map.

use super::extract::Credentials;
use axum::Json;
use axum::extract::{Path, Query, State};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::ApiResult;
use super::extract::{ShareQuery, read_map_as};
use crate::auth::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(map_killmails))
}

/// `GET /api/maps/{id}/killmails`, recent kills in this map's systems, newest first.
/// Viewer+, like reading the graph: a killmail is public record on zKillboard anyway.
#[utoipa::path(
    get,
    path = "/api/maps/{id}/killmails",
    tag = "killmails",
    params(("id" = i64, Path, description = "The map"), ShareQuery),
    responses((status = 200, body = Vec<crate::killmails::MapKillmail>, description = "OK"), (status = 401, response = super::Unauthorized), (status = 404, response = super::NotFound)),
    security((), ("bearer" = []), ("session" = [])),
)]
pub async fn map_killmails(
    State(state): State<AppState>,
    creds: Credentials,
    Path(map_id): Path<i64>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<crate::killmails::MapKillmail>> {
    let reader = read_map_as(&state, &creds, map_id, &share).await?;
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
