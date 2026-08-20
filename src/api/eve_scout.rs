//! The public EVE Scout feed: Thera and Turnur connections, refreshed on demand.

use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::routing::get;
use serde::{Deserialize, Serialize};

use super::ApiResult;
use crate::auth::AppState;

/// A public wormhole out of Thera or Turnur, as EVE Scout's scouts have it. Oriented
/// hub-first rather than in EVE Scout's in/out terms, and statuses normalized to WormholeSystems's
/// own vocabulary.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct EveScoutConnection {
    pub hub_solar_system_id: i64,
    /// `Thera` or `Turnur`.
    pub hub: String,
    pub hub_signature: String,
    pub solar_system_id: i64,
    pub signature: String,
    pub mass_status: String,
    pub time_status: String,
    /// The wormhole code, e.g. `J377`. Absent while a scout has only half-scanned it.
    #[ts(optional)]
    pub wormhole_type: Option<String>,
    /// `frigate` / `medium` / `large` / `capital`.
    #[ts(optional)]
    pub max_ship_size: Option<String>,
    #[ts(optional)]
    pub remaining_hours: Option<f64>,
    /// When a scout last touched it, for the card's freshness stamp.
    #[ts(optional)]
    pub updated_at: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/evescout", get(eve_scout))
}

/// Thera and Turnur, the two systems EVE Scout keeps public connections for.
const HUBS: [(i64, &str); 2] = [(31000005, "Thera"), (30002086, "Turnur")];

/// Normalize one EVE Scout signature. Tolerant of shape drift: unknown fields default to
/// healthy/fresh, and a row without both endpoints, or with neither end at a hub, is
/// dropped rather than guessed at.
pub(crate) fn eve_scout_connection(sig: &serde_json::Value) -> Option<EveScoutConnection> {
    let in_id = sig.get("in_system_id")?.as_i64()?;
    let out_id = sig.get("out_system_id")?.as_i64()?;
    let text = |key: &str| {
        sig.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    let (hub_solar_system_id, hub) = HUBS
        .iter()
        .find(|(id, _)| *id == in_id || *id == out_id)
        .map(|(id, name)| (*id, (*name).to_string()))?;
    let hub_is_in = hub_solar_system_id == in_id;
    let (hub_signature, signature) = if hub_is_in {
        (text("in_signature"), text("out_signature"))
    } else {
        (text("out_signature"), text("in_signature"))
    };
    let mass = match sig.get("mass").and_then(|v| v.as_str()).unwrap_or("") {
        m if m.contains("crit") => "critical",
        m if m.contains("reduced") || m.contains("destab") => "reduced",
        _ => "stable",
    };
    let time = match sig.get("life").and_then(|v| v.as_str()) {
        Some(l) if l.contains("crit") => "critical",
        Some(l) if l.contains("eol") => "eol",
        Some(_) => "stable",
        // No life field: derive from the remaining lifetime when present.
        None => match sig.get("remaining_hours").and_then(|v| v.as_f64()) {
            Some(h) if h < 1.0 => "critical",
            Some(h) if h < 4.0 => "eol",
            _ => "stable",
        },
    };
    Some(EveScoutConnection {
        hub_solar_system_id,
        hub,
        hub_signature: hub_signature.unwrap_or_default(),
        solar_system_id: if hub_is_in { out_id } else { in_id },
        signature: signature.unwrap_or_default(),
        mass_status: mass.into(),
        time_status: time.into(),
        wormhole_type: text("wh_type"),
        max_ship_size: text("max_ship_size"),
        remaining_hours: sig.get("remaining_hours").and_then(|v| v.as_f64()),
        updated_at: text("updated_at").or_else(|| text("created_at")),
    })
}

/// `GET /api/evescout`, public Thera/Turnur connections, proxied and cached for 60s.
/// Upstream failures degrade to an empty list.
pub async fn eve_scout(State(_state): State<AppState>) -> ApiResult<Vec<EveScoutConnection>> {
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    type Cached = Option<(Instant, Vec<EveScoutConnection>)>;
    static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));

    if let Some((at, edges)) = cache.lock().expect("cache lock").as_ref()
        && at.elapsed() < Duration::from_secs(60)
    {
        return Ok(Json(edges.clone()));
    }

    let edges = fetch_eve_scout().await.unwrap_or_default();
    *cache.lock().expect("cache lock") = Some((Instant::now(), edges.clone()));
    Ok(Json(edges))
}

async fn fetch_eve_scout() -> Option<Vec<EveScoutConnection>> {
    let client = reqwest::Client::builder()
        .user_agent(crate::user_agent::get())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    // Overridable so the e2e suite can serve a fixed set: the real list is scouted by hand
    // and changes under the tests.
    let url = std::env::var("EVE_SCOUT_URL")
        .unwrap_or_else(|_| "https://api.eve-scout.com/v2/public/signatures".to_string());
    let body: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    Some(
        body.as_array()?
            .iter()
            .filter_map(eve_scout_connection)
            .collect(),
    )
}

#[cfg(test)]
mod eve_scout_tests {
    use super::eve_scout_connection;

    #[test]
    fn normalizes_statuses_and_drops_incomplete_rows() {
        let sig = serde_json::json!({
            "in_system_id": 30000142, "out_system_id": 31000005,
            "mass": "destab", "life": "eol"
        });
        let edge = eve_scout_connection(&sig).unwrap();
        assert_eq!(edge.mass_status, "reduced");
        assert_eq!(edge.time_status, "eol");

        let sig = serde_json::json!({
            "in_system_id": 30000142, "out_system_id": 31000005,
            "remaining_hours": 0.5
        });
        let edge = eve_scout_connection(&sig).unwrap();
        assert_eq!(edge.mass_status, "stable");
        assert_eq!(edge.time_status, "critical");

        let sig = serde_json::json!({ "in_system_id": 30000142 });
        assert!(eve_scout_connection(&sig).is_none());
    }

    /// Whichever end is the hub, the row comes back hub-first.
    #[test]
    fn orients_the_hub_first_either_way() {
        let a = eve_scout_connection(&serde_json::json!({
            "in_system_id": 31001882, "out_system_id": 30002086,
            "in_signature": "BCC-784", "out_signature": "DZT-829", "wh_type": "J377"
        }))
        .unwrap();
        assert_eq!(a.hub, "Turnur");
        assert_eq!(a.hub_solar_system_id, 30002086);
        assert_eq!(a.solar_system_id, 31001882);
        assert_eq!(a.hub_signature, "DZT-829");
        assert_eq!(a.signature, "BCC-784");
        assert_eq!(a.wormhole_type.as_deref(), Some("J377"));

        let b = eve_scout_connection(&serde_json::json!({
            "in_system_id": 31000005, "out_system_id": 30000142,
            "in_signature": "AAA-111", "out_signature": "BBB-222"
        }))
        .unwrap();
        assert_eq!(b.hub, "Thera");
        assert_eq!(b.solar_system_id, 30000142);
        assert_eq!(b.hub_signature, "AAA-111");
        assert_eq!(b.signature, "BBB-222");

        // Neither end is a hub: EVE Scout only publishes Thera and Turnur, so this is drift.
        assert!(
            eve_scout_connection(&serde_json::json!({
                "in_system_id": 30000142, "out_system_id": 30002187
            }))
            .is_none()
        );
    }
}
