use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use axum::extract::{FromRef, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use leptos::prelude::LeptosOptions;
use serde::Deserialize;

use crate::esi::{EsiClient, Sso};

#[derive(Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub auth: Arc<Auth>,
    pub db: sqlx::PgPool,
}

impl FromRef<AppState> for LeptosOptions {
    fn from_ref(state: &AppState) -> Self {
        state.leptos_options.clone()
    }
}

pub struct Auth {
    sso: Sso,
    esi: EsiClient,
    // In-memory CSRF states — a dev stand-in for the `oauth_login_flows` table.
    states: Mutex<HashSet<String>>,
}

impl Auth {
    pub fn new(sso: Sso, esi: EsiClient) -> Self {
        Auth {
            sso,
            esi,
            states: Mutex::new(HashSet::new()),
        }
    }

    fn issue_state(&self) -> String {
        let state = uuid::Uuid::new_v4().to_string();
        self.states.lock().unwrap().insert(state.clone());
        state
    }

    fn take_state(&self, state: &str) -> bool {
        self.states.lock().unwrap().remove(state)
    }
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: String,
}

pub async fn login(State(state): State<AppState>) -> Response {
    let csrf = state.auth.issue_state();
    Redirect::to(&state.auth.sso.authorize_url(&csrf)).into_response()
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let auth = &state.auth;
    if !auth.take_state(&query.state) {
        return (StatusCode::BAD_REQUEST, "invalid or expired state").into_response();
    }
    let (_token, claims) = match auth.sso.exchange_code(&query.code).await {
        Ok(pair) => pair,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("login failed: {e}")).into_response(),
    };

    // Resolve the character's affiliations (public endpoints) for display. No
    // persistence yet — everything is passed to /profile via the query string.
    let mut params: Vec<(&str, String)> = vec![
        ("name", claims.name.clone()),
        ("id", claims.character_id.to_string()),
    ];
    if !claims.scopes.is_empty() {
        params.push(("scopes", claims.scopes.join(" ")));
    }
    if let Ok(Some(aff)) = auth
        .esi
        .affiliation(&[claims.character_id])
        .await
        .map(|a| a.into_iter().next())
    {
        if let Ok(corp) = auth.esi.corporation(aff.corporation_id).await {
            params.push(("corporation", format!("{} [{}]", corp.name, corp.ticker)));
        }
        if let Some(alliance_id) = aff.alliance_id
            && let Ok(alliance) = auth.esi.alliance(alliance_id).await
        {
            params.push((
                "alliance",
                format!("{} [{}]", alliance.name, alliance.ticker),
            ));
        }
        if let Some(faction_id) = aff.faction_id {
            params.push(("faction", faction_id.to_string()));
        }
    }

    let dest = reqwest::Url::parse_with_params("http://x/profile", &params).expect("valid url");
    Redirect::to(&format!("/profile?{}", dest.query().unwrap_or_default())).into_response()
}
