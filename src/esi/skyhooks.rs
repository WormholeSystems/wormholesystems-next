use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{EsiClient, Result};

#[derive(Debug, Clone, Deserialize)]
struct RaidableSkyhooks {
    skyhooks: Vec<RaidableSkyhook>,
}

/// A skyhook whose theft window is open or about to open. Unauthenticated: CCP publishes
/// the raidable set to everyone, which is the whole point of the endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct RaidableSkyhook {
    pub planet_id: i64,
    pub solar_system_id: i64,
    pub theft_vulnerability: TheftWindow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TheftWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl EsiClient {
    /// Every skyhook currently or shortly raidable, across New Eden.
    pub async fn raidable_skyhooks(&self) -> Result<Vec<RaidableSkyhook>> {
        let body: RaidableSkyhooks = self.get_json("/skyhooks/raidable", None).await?;
        Ok(body.skyhooks)
    }
}
