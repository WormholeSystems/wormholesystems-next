//! Application configuration, read from the environment (and a `.env` file in dev).

use crate::esi::{Scope, SsoConfig};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub sso: SsoConfig,
}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        // Load `.env` if present; real environment variables take precedence.
        dotenvy::dotenv().ok();

        Ok(Config {
            sso: SsoConfig {
                client_id: required("EVE_CLIENT_ID")?,
                client_secret: required("EVE_CLIENT_SECRET")?,
                redirect_uri: required("EVE_REDIRECT_URI")?,
                scopes: Scope::ALL.to_vec(),
            },
        })
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::Missing(key))
}
