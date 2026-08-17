use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{EsiClient, Result};

/// Tranquility's own report of itself. Unauthenticated.
#[derive(Debug, Clone, Deserialize)]
pub struct TranquilityStatus {
    pub players: i64,
    pub server_version: String,
    pub start_time: DateTime<Utc>,
    /// VIP mode: the server is up, but only CCP can log in. Absent means no.
    #[serde(default)]
    pub vip: bool,
}

impl EsiClient {
    pub async fn server_status(&self) -> Result<TranquilityStatus> {
        self.get_json("/status", None).await
    }
}
