//! Who may see a map and what they may do there: grants, ownership, and the share
//! tokens that let somebody watch without an account.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::ApiResult;
use super::extract::{acting_on, require_actor};
use super::reference::SearchQuery;
use crate::auth::AppState;
use crate::maps::MapEvent;
use crate::maps::access::{AccessEntry, RevokeAccess, SetAccess};

/// A grantable subject from the access-subject search.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AccessSubject {
    pub subject_type: crate::maps::SubjectType,
    pub subject_id: i64,
    pub name: String,
    pub ticker: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/access-subjects/search", get(search_access_subjects))
        .route("/api/maps/{id}/access", get(list_access))
        .route("/api/maps/{id}/access/set", post(set_access))
        .route("/api/maps/{id}/access/revoke", post(revoke_access))
        .route("/api/maps/{id}/access/transfer", post(transfer_ownership))
        .route(
            "/api/maps/{id}/share",
            post(rotate_share_token).delete(revoke_share_token),
        )
}

/// `GET /api/access-subjects/search?q=`, characters, corporations and alliances that can
/// be granted access. Only entities Vector has already cached are searchable (a character
/// who has signed in, or a corp/alliance one of them belongs to), hence the UI also
/// accepting a raw EVE id.
pub async fn search_access_subjects(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<SearchQuery>,
) -> ApiResult<Vec<AccessSubject>> {
    require_actor(&state.db, &jar).await?;
    let q = query.q.trim();
    if q.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");
    let rows = sqlx::query!(
        // The `!` overrides restate what the source tables guarantee: sqlx cannot see
        // through the union and widens every column to nullable.
        r#"select id as "id!", name as "name!",
                  kind as "kind!: crate::maps::SubjectType", ticker
           from (
               select id, name, 'character' as kind, null::text as ticker from characters
               union all
               select id, name, 'corporation', ticker from corporations
               union all
               select id, name, 'alliance', ticker from alliances
           ) s
           where s.name ilike $1 or s.ticker ilike $1
           order by (s.name ilike $2) desc, length(s.name), s.name
           limit 20"#,
        contains,
        prefix,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| AccessSubject {
                subject_type: r.kind,
                subject_id: r.id,
                name: r.name,
                ticker: r.ticker,
            })
            .collect(),
    ))
}

/// `GET /api/maps/{id}/access`, who can see this map, and at what role. Viewer+.
pub async fn list_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<Vec<AccessEntry>> {
    let actor = require_actor(&state.db, &jar).await?;
    let entries = crate::maps::access::list_access(&state.db, actor, map_id).await?;
    Ok(Json(entries))
}

pub async fn set_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<SetAccess>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    crate::maps::access::set_access(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::AccessChanged { map_id });
    Ok(Json(()))
}

pub async fn revoke_access(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RevokeAccess>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    crate::maps::access::revoke_access(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::AccessChanged { map_id });
    Ok(Json(()))
}

/// `POST /api/maps/{id}/access/transfer`, hand the map to another character on it.
pub async fn transfer_ownership(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<crate::maps::access::TransferOwnership>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    crate::maps::access::transfer_ownership(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::AccessChanged { map_id });
    Ok(Json(()))
}

/// `POST /api/maps/{id}/share`, mint a share link, replacing any earlier one.
pub async fn rotate_share_token(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<String> {
    let actor = require_actor(&state.db, &jar).await?;
    let token = crate::maps::map::rotate_share_token(&state.db, actor, map_id).await?;
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(token))
}

/// `DELETE /api/maps/{id}/share`, withdraw the share link.
pub async fn revoke_share_token(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<()> {
    let actor = require_actor(&state.db, &jar).await?;
    crate::maps::map::revoke_share_token(&state.db, actor, map_id).await?;
    state.hub.publish(MapEvent::MapUpdated { map_id });
    Ok(Json(()))
}
