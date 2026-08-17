//! Application configuration, read from the environment (and a `.env` file in dev).

use crate::esi::{Scope, SsoConfig};
use crate::maps::GridConfig;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub sso: SsoConfig,
    pub grid: GridConfig,
    /// Where ESI lives. Overridable so the e2e suite can point the whole stack at a stub
    /// and drive a pilot around without a live EVE session.
    pub esi_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        // Load `.env` if present; real environment variables take precedence.
        dotenvy::dotenv().ok();

        Ok(Config {
            database_url: required("DATABASE_URL")?,
            sso: SsoConfig {
                client_id: required("EVE_CLIENT_ID")?,
                client_secret: required("EVE_CLIENT_SECRET")?,
                redirect_uri: required("EVE_REDIRECT_URI")?,
                scopes: Scope::ALL.to_vec(),
            },
            grid: grid_from_env(),
            esi_base_url: std::env::var("ESI_BASE_URL")
                .unwrap_or_else(|_| crate::esi::BASE_URL.to_string()),
        })
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::Missing(key))
}

/// Build the map [`GridConfig`] from the environment; each field falls back to its default
/// when the var is unset or unparseable.
fn grid_from_env() -> GridConfig {
    let d = GridConfig::default();
    GridConfig {
        cell_size: optional_f64("GRID_CELL_SIZE", d.cell_size),
        world_width: optional_f64("GRID_WORLD_WIDTH", d.world_width),
        world_height: optional_f64("GRID_WORLD_HEIGHT", d.world_height),
        viewport_height: optional_f64("GRID_VIEWPORT_HEIGHT", d.viewport_height),
    }
}

fn optional_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
