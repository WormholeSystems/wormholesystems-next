//! Recording a jump a pilot has made, which places, connects and links in one step.

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::post;
use axum_extra::extract::CookieJar;

use super::ApiResult;
use super::extract::acting_on;
use crate::auth::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/maps/{id}/track-jump", post(track_jump))
}

/// `POST /api/maps/{id}/track-jump`, record a jump: place the system, connect it, and
/// link the signature it turned out to be. Member+. One command, so it undoes as one step;
/// it can touch a system, a connection and a signature at once, so clients just refetch.
pub async fn track_jump(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<crate::maps::tracking::TrackJump>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    crate::maps::tracking::track_jump(&state.db, actor, cmd).await?;
    Ok(Json(()))
}
