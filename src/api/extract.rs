//! Request plumbing every area shares: who is calling, what they are allowed to be
//! reading, and the small checks that guard a command before it reaches an action.

use axum_extra::extract::CookieJar;
use serde::Deserialize;

use super::ApiError;
use crate::auth::AppState;
use crate::maps::Actor;

/// The raw session id from the cookie, if any.
pub(crate) fn session_id(jar: &CookieJar) -> Option<String> {
    jar.get(crate::session::SESSION_COOKIE)
        .map(|c| c.value().to_string())
}

/// The acting character from the session cookie, or `None` if not signed in.
pub(crate) async fn session_actor(
    db: &sqlx::PgPool,
    jar: &CookieJar,
) -> Result<Option<Actor>, ApiError> {
    let Some(session_id) = session_id(jar) else {
        return Ok(None);
    };
    Ok(crate::session::actor_for_session(db, &session_id).await?)
}

/// The auth guard: the acting character, or 401.
pub(crate) async fn require_actor(db: &sqlx::PgPool, jar: &CookieJar) -> Result<Actor, ApiError> {
    session_actor(db, jar)
        .await?
        .ok_or_else(ApiError::unauthorized)
}

/// Command bodies carry `map_id` (the action contracts authorize on it); it must agree
/// with the path so a URL can't act on a different map than it names.
fn check_map_id(path_id: i64, body_id: i64) -> Result<(), ApiError> {
    if path_id == body_id {
        Ok(())
    } else {
        Err(ApiError::bad_request("map id in body does not match URL"))
    }
}

/// The caller, for a command that names its own map: the id in the body has to agree with
/// the one in the URL before the session is resolved, so a URL can never act on a map it
/// does not name. Every mutating handler starts here.
pub(crate) async fn acting_on(
    db: &sqlx::PgPool,
    jar: &CookieJar,
    path_id: i64,
    body_id: i64,
) -> Result<Actor, ApiError> {
    check_map_id(path_id, body_id)?;
    require_actor(db, jar).await
}

/// The share token, when the caller has one.
#[derive(Deserialize)]
pub struct ShareQuery {
    #[serde(default)]
    pub share: Option<String>,
}

/// The token the share route left behind, so following a link once does not mean carrying
/// the token in every address afterwards.
pub(crate) fn share_cookie(jar: &CookieJar, map_id: i64) -> Option<String> {
    jar.get(&format!("map_share_{map_id}"))
        .map(|c| c.value().to_string())
        .filter(|token| !token.is_empty())
}

/// Who is reading: a grant if the session has one, otherwise whatever the map has been
/// opened up to. Guests never get further than viewer.
pub(crate) async fn read_map_as(
    state: &AppState,
    jar: &CookieJar,
    map_id: i64,
    share: &ShareQuery,
) -> Result<crate::maps::access::Reader, ApiError> {
    let actor = session_actor(&state.db, jar).await?;
    let token = share
        .share
        .clone()
        .or_else(|| share_cookie(jar, map_id))
        .filter(|token| !token.is_empty());
    Ok(crate::maps::access::reader_for(&state.db, map_id, actor, token.as_deref()).await?)
}
