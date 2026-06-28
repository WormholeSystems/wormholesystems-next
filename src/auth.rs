use std::sync::Arc;

use axum::extract::{FromRef, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use leptos::prelude::LeptosOptions;
use serde::Deserialize;
use uuid::Uuid;

use crate::db::PgTokenStore;
use crate::esi::token::TokenStore;
use crate::esi::{EsiClient, Sso};
use crate::session;

#[derive(Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub auth: Arc<Auth>,
    pub db: sqlx::PgPool,
    pub hub: crate::maps::MapHub,
    pub user_hub: crate::user_channel::UserHub,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

pub struct Auth {
    sso: Arc<Sso>,
    esi: EsiClient,
}

impl Auth {
    pub fn new(sso: Arc<Sso>, esi: EsiClient) -> Self {
        Auth { sso, esi }
    }
}

/// `GET /auth/login` — record a one-time CSRF `state` and redirect to the EVE SSO.
pub async fn login(State(state): State<AppState>) -> Response {
    let csrf = Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query!(
        "insert into oauth_login_flows (state, expires_at)
         values ($1, now() + interval '10 minutes')",
        csrf,
    )
    .execute(&state.db)
    .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not start login: {err}"),
        )
            .into_response();
    }
    Redirect::to(&state.auth.sso.authorize_url(&csrf)).into_response()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

/// `GET /auth/callback` — validate the handshake, persist the character + token, open a
/// session, set the cookie, and redirect into the app.
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let auth = &state.auth;

    // The `state` is single-use and short-lived: consume it, rejecting unknown/expired.
    let flow = match sqlx::query!(
        "delete from oauth_login_flows where state = $1 and expires_at > now()
         returning link_user_id, redirect_to",
        query.state,
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(flow)) => flow,
        Ok(None) => return (StatusCode::BAD_REQUEST, "invalid or expired state").into_response(),
        Err(err) => return server_error(err),
    };

    let (token, claims) = match auth.sso.exchange_code(&query.code).await {
        Ok(pair) => pair,
        Err(err) => {
            return (StatusCode::BAD_GATEWAY, format!("login failed: {err}")).into_response();
        }
    };

    // Corp/alliance drive access checks, so they're required and refreshed on every login.
    // We also cache the entity rows (name/ticker) the character's deferred FKs reference.
    let affiliation = auth
        .esi
        .affiliation(&[claims.character_id])
        .await
        .ok()
        .and_then(|a| a.into_iter().next());
    let Some(affiliation) = affiliation else {
        return (
            StatusCode::BAD_GATEWAY,
            "could not resolve character affiliation",
        )
            .into_response();
    };

    let Ok(corp) = auth.esi.corporation(affiliation.corporation_id).await else {
        return (StatusCode::BAD_GATEWAY, "could not resolve corporation").into_response();
    };
    let corporation = session::Entity {
        id: affiliation.corporation_id,
        name: corp.name,
        ticker: corp.ticker,
    };
    let alliance = match affiliation.alliance_id {
        Some(alliance_id) => match auth.esi.alliance(alliance_id).await {
            Ok(a) => Some(session::Entity {
                id: alliance_id,
                name: a.name,
                ticker: a.ticker,
            }),
            Err(_) => {
                return (StatusCode::BAD_GATEWAY, "could not resolve alliance").into_response();
            }
        },
        None => None,
    };

    let user_id = match session::persist_identity(
        &state.db,
        &claims,
        corporation,
        alliance,
        flow.link_user_id,
    )
    .await
    {
        Ok(user_id) => user_id,
        Err(err) => return server_error(err),
    };

    // Persist the ESI token for this character (refresh token is the sensitive credential).
    if let Err(err) = PgTokenStore::new(state.db.clone())
        .save(claims.character_id, &token)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("could not store token: {err}"),
        )
            .into_response();
    }

    let session_id = match session::create_session(&state.db, user_id, claims.character_id).await {
        Ok(id) => id,
        Err(err) => return server_error(err),
    };

    let cookie = Cookie::build((session::SESSION_COOKIE, session_id))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        // TODO: `.secure(true)` once served over HTTPS (off for local http dev).
        .build();
    let destination = flow.redirect_to.unwrap_or_else(|| "/".to_string());
    (jar.add(cookie), Redirect::to(&destination)).into_response()
}

/// Route guard middleware: redirect unauthenticated requests for protected paths
/// (`/maps`, `/maps/...`) to the login page before any rendering happens. Server functions
/// enforce auth independently; this is the page-level gate.
pub async fn require_login(
    State(state): State<AppState>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let protected = path == "/maps" || path.starts_with("/maps/");
    if protected {
        let authed = match jar.get(session::SESSION_COOKIE) {
            Some(cookie) => session::actor_for_session(&state.db, cookie.value())
                .await
                .ok()
                .flatten()
                .is_some(),
            None => false,
        };
        if !authed {
            return Redirect::to("/login").into_response();
        }
    }
    next.run(req).await
}

/// `GET /auth/logout` — end the session and clear the cookie.
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(cookie) = jar.get(session::SESSION_COOKIE) {
        let _ = session::delete_session(&state.db, cookie.value()).await;
    }
    (
        jar.remove(Cookie::from(session::SESSION_COOKIE)),
        Redirect::to("/"),
    )
        .into_response()
}

fn server_error(err: sqlx::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("database error: {err}"),
    )
        .into_response()
}
