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
    /// Absent when the application has no Discord app configured, which is the normal
    /// state for a dev machine: the alerts UI still works, the bot half simply is not there.
    pub discord: Option<DiscordConfig>,
}

/// What the Discord half of the integration needs.
///
/// All or nothing: a half-configured app fails in ways that look like bugs, so this is read
/// as one unit and left `None` unless every part is present.
#[derive(Debug, Clone)]
pub struct DiscordConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    /// Verifies that an interaction really came from Discord. Hex, from the app's page.
    pub public_key: String,
    /// Only needed to post as the bot; account linking and slash commands work without it.
    pub bot_token: Option<String>,
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
            discord: discord_from_env(),
        })
    }
}

fn discord_from_env() -> Option<DiscordConfig> {
    Some(DiscordConfig {
        client_id: std::env::var("DISCORD_CLIENT_ID").ok()?,
        client_secret: std::env::var("DISCORD_CLIENT_SECRET").ok()?,
        redirect_uri: std::env::var("DISCORD_REDIRECT_URI").ok()?,
        public_key: std::env::var("DISCORD_PUBLIC_KEY").ok()?,
        bot_token: std::env::var("DISCORD_BOT_TOKEN").ok(),
    })
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
