//! Async client for the EVE Swagger Interface (ESI) and its SSO.
//!
//! `EsiClient` is framework-agnostic: authenticated calls take an access-token string,
//! and the web layer supplies/persists tokens through [`token::TokenStore`].

use serde::Serialize;
use serde::de::DeserializeOwned;

pub mod error;
pub mod jwt;
pub mod scopes;
pub mod sso;
pub mod token;

pub mod character;
pub mod entities;
pub mod skyhooks;
pub mod sovereignty;
pub mod status;
pub mod ui;

#[allow(unused_imports)]
pub use {
    character::{CharacterLocation, CharacterOnline, CharacterPublic, CharacterShip},
    entities::{Affiliation, Alliance, Corporation},
    error::{EsiError, Result},
    jwt::Claims,
    scopes::Scope,
    skyhooks::RaidableSkyhook,
    sovereignty::SovereigntySystem,
    sso::{Sso, SsoConfig},
    status::TranquilityStatus,
    token::{Token, TokenStore},
};

pub const BASE_URL: &str = "https://esi.evetech.net";
pub const COMPATIBILITY_DATE: &str = "2026-06-09";

#[derive(Clone)]
pub struct EsiClient {
    http: reqwest::Client,
    base_url: String,
    compatibility_date: String,
}

impl EsiClient {
    pub fn new() -> Self {
        Self::with_config(reqwest::Client::new(), BASE_URL, COMPATIBILITY_DATE)
    }

    pub fn with_config(
        http: reqwest::Client,
        base_url: impl Into<String>,
        compatibility_date: impl Into<String>,
    ) -> Self {
        EsiClient {
            http,
            base_url: base_url.into(),
            compatibility_date: compatibility_date.into(),
        }
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: Option<&str>,
    ) -> reqwest::RequestBuilder {
        let mut req = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("X-Compatibility-Date", &self.compatibility_date);
        if let Some(token) = token {
            req = req.bearer_auth(token);
        }
        req
    }

    /// Send a request, returning the response or an [`EsiError::Api`] that captures the body
    /// on a non-success status, which `reqwest`'s `error_for_status` discards.
    async fn send_checked(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(EsiError::Api {
                status: status.as_u16(),
                body,
            });
        }
        Ok(resp)
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &str, token: Option<&str>) -> Result<T> {
        let resp = self
            .send_checked(self.request(reqwest::Method::GET, path, token))
            .await?;
        Ok(resp.json().await?)
    }

    async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        token: Option<&str>,
    ) -> Result<T> {
        let resp = self
            .send_checked(self.request(reqwest::Method::POST, path, token).json(body))
            .await?;
        Ok(resp.json().await?)
    }
}

impl Default for EsiClient {
    fn default() -> Self {
        Self::new()
    }
}
