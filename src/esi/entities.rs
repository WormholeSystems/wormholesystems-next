use serde::Deserialize;

use super::{EsiClient, Result};

#[derive(Debug, Clone, Deserialize)]
pub struct Corporation {
    pub name: String,
    pub ticker: String,
    pub alliance_id: Option<i64>,
    pub member_count: i64,
    pub ceo_id: i64,
    pub creator_id: i64,
    pub tax_rate: f64,
    pub date_founded: Option<String>,
    pub faction_id: Option<i64>,
    pub home_station_id: Option<i64>,
    pub url: Option<String>,
    pub war_eligible: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Alliance {
    pub name: String,
    pub ticker: String,
    pub creator_corporation_id: i64,
    pub creator_id: i64,
    pub date_founded: String,
    pub executor_corporation_id: Option<i64>,
    pub faction_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Affiliation {
    pub character_id: i64,
    pub corporation_id: i64,
    pub alliance_id: Option<i64>,
    pub faction_id: Option<i64>,
}

impl EsiClient {
    pub async fn corporation(&self, corporation_id: i64) -> Result<Corporation> {
        self.get_json(&format!("/corporations/{corporation_id}"), None)
            .await
    }

    pub async fn alliance(&self, alliance_id: i64) -> Result<Alliance> {
        self.get_json(&format!("/alliances/{alliance_id}"), None)
            .await
    }

    /// ESI accepts at most 1000 ids per call; callers batch larger sets.
    pub async fn affiliation(&self, character_ids: &[i64]) -> Result<Vec<Affiliation>> {
        self.post_json("/characters/affiliation", &character_ids, None)
            .await
    }
}
