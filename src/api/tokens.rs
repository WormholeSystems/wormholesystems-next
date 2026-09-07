//! The account's personal access tokens. Session-only on purpose: see
//! [`require_session_actor`](super::extract::require_session_actor).

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{delete, get};

use super::ApiResult;
use super::extract::{Credentials, require_session_actor};
use crate::auth::AppState;
use crate::tokens::{CreateToken, CreatedToken, PersonalAccessToken};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/me/tokens", get(my_tokens).post(create_token))
        .route("/api/me/tokens/{id}", delete(revoke_token))
}

/// `GET /api/me/tokens`, every token on the account, never the secrets.
pub async fn my_tokens(
    State(state): State<AppState>,
    creds: Credentials,
) -> ApiResult<Vec<PersonalAccessToken>> {
    let actor = require_session_actor(&state.db, &creds).await?;
    Ok(Json(crate::tokens::list(&state.db, actor.user_id).await?))
}

/// `POST /api/me/tokens`, mint one. The plaintext is in this response and nowhere else.
pub async fn create_token(
    State(state): State<AppState>,
    creds: Credentials,
    Json(cmd): Json<CreateToken>,
) -> ApiResult<CreatedToken> {
    let actor = require_session_actor(&state.db, &creds).await?;
    Ok(Json(
        crate::tokens::create(&state.db, actor.user_id, cmd).await?,
    ))
}

/// `DELETE /api/me/tokens/{id}`, and whatever used it is locked out on its next request.
pub async fn revoke_token(
    State(state): State<AppState>,
    creds: Credentials,
    Path(token_id): Path<i64>,
) -> ApiResult<()> {
    let actor = require_session_actor(&state.db, &creds).await?;
    crate::tokens::revoke(&state.db, actor.user_id, token_id).await?;
    Ok(Json(()))
}
