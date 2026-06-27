use serde::Deserialize;

use super::{EsiClient, Result};

#[derive(Debug, Clone, Deserialize)]
struct SovereigntyMap {
    solar_systems: Vec<SovereigntySystem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SovereigntySystem {
    pub solar_system_id: i64,
    pub claim: SovereigntyClaim,
}

/// Exactly one of `faction` / `alliance` / `unclaimed` is set.
#[derive(Debug, Clone, Deserialize)]
pub struct SovereigntyClaim {
    pub faction: Option<FactionClaim>,
    pub alliance: Option<AllianceClaim>,
    pub unclaimed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactionClaim {
    pub faction_id: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AllianceClaim {
    pub alliance_id: i64,
    pub corporation_id: i64,
    pub claimed_since: Option<String>,
    pub is_capital_system: Option<bool>,
}

impl EsiClient {
    pub async fn sovereignty_systems(&self) -> Result<Vec<SovereigntySystem>> {
        let map: SovereigntyMap = self.get_json("/sovereignty/systems", None).await?;
        Ok(map.solar_systems)
    }
}
