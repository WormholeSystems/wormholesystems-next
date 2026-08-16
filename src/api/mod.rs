//! The JSON HTTP API: plain Axum handlers over the [`crate::maps`] actions, plus the
//! realtime WebSocket handlers. This is the client/server boundary for the SvelteKit
//! frontend.
//!
//! The acting [`Actor`](crate::maps::Actor) is resolved **from the session cookie**
//! server-side — never sent by the client. Each mutating handler publishes the matching
//! [`MapEvent`](crate::maps::MapEvent) to the hub after the action commits.

pub mod handlers;
pub mod ws;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;
use serde_json::json;

use serde::{Deserialize, Serialize};

use crate::auth::AppState;
use crate::maps::{Actor, MapError};

/// The signed-in character, for the UI's auth state.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CharacterSummary {
    pub character_id: i64,
    pub name: String,
}

/// Live status of the active character, for the navbar readout.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CharacterStatus {
    pub online: bool,
    pub solar_system: Option<String>,
    pub ship_type_id: Option<i64>,
    pub ship_name: Option<String>,
    pub ship_type: Option<String>,
}

/// One of the user's characters, for the switcher.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct CharacterRef {
    pub character_id: i64,
    pub name: String,
    pub is_active: bool,
    pub online: bool,
    /// Where the character is right now, when online and tracked. Drives the paste
    /// system-mismatch warning.
    pub solar_system_id: Option<i64>,
}

/// A map in the user's list, with their role on it.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapEntry {
    pub id: i64,
    pub name: String,
    pub role: String,
}

/// A solar system matched by the "add system" search, with just enough to display and pick.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SystemSearchResult {
    pub id: i64,
    pub name: String,
    pub security: f64,
    pub region: String,
    pub region_id: i64,
    pub constellation_id: i64,
    pub wormhole_class_id: Option<i32>,
    /// Wormhole effect, for J-space rows (shown where k-space rows show sovereignty).
    pub effect_name: Option<String>,
    pub sovereignty: Option<crate::maps::solar_system::Sovereignty>,
}

/// A user's per-map preferences.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapUserSettings {
    pub tracking_allowed: bool,
    pub show_threat_level: bool,
    pub compact_signature_list: bool,
    pub show_statics_first: bool,
    /// `shorter` / `safer` / `less_secure`.
    pub route_preference: String,
    /// 0-100, weight of the security preference (legacy `exp(0.15 * penalty)`).
    pub security_penalty: i32,
    /// Worst wormhole lifetime still routed through: `stable` / `eol` / `critical`.
    pub route_allow_time_status: String,
    /// Worst wormhole mass still routed through: `stable` / `reduced` / `critical`.
    pub route_allow_mass_status: String,
    pub route_use_evescout: bool,
}

/// Partial update of [`MapUserSettings`]; absent fields stay unchanged.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct UpdateMapUserSettings {
    #[serde(default)]
    #[ts(optional)]
    pub tracking_allowed: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub show_threat_level: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub compact_signature_list: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub show_statics_first: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub route_preference: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub security_penalty: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub route_allow_time_status: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub route_allow_mass_status: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub route_use_evescout: Option<bool>,
}

/// A public Thera/Turnur wormhole edge from EVE Scout, normalized to Vector's status
/// vocabulary for the client-side router.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct EveScoutEdge {
    pub from_solar_system_id: i64,
    pub to_solar_system_id: i64,
    pub mass_status: String,
    pub time_status: String,
}

/// A cosmic-signature category from the seeded catalog.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SignatureCategoryInfo {
    pub id: i64,
    pub name: String,
    pub code: String,
}

/// A cosmic-signature type from the seeded catalog. `signature` is the wormhole code
/// (wormhole types only); `target_class` its destination class; `spawn_areas` the system
/// classes this type can appear in.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SignatureTypeInfo {
    pub id: i64,
    pub signature: Option<String>,
    pub name: String,
    pub signature_category_id: i64,
    pub target_class: Option<i32>,
    pub extra: Option<String>,
    pub spawn_areas: Vec<i32>,
    /// Wormhole physics (joined from `wormhole_types` by code; wormhole types only).
    pub total_mass: Option<i64>,
    pub max_jump_mass: Option<i64>,
    pub lifetime_hours: Option<f64>,
    pub signature_strength: Option<f64>,
}

/// A ship type matched by the manual-jump ship search.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ShipSearchResult {
    pub id: i64,
    pub name: String,
    pub group_name: String,
    /// Hull mass in kg.
    pub mass: Option<f64>,
}

/// The full signature catalog, served once and cached client-side.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SignatureCatalog {
    pub categories: Vec<SignatureCategoryInfo>,
    pub types: Vec<SignatureTypeInfo>,
}

/// An online, tracked character on the map (presence), for the node pilot rows.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapCharacter {
    pub character_id: i64,
    pub name: String,
    pub corporation_ticker: String,
    pub solar_system_id: Option<i64>,
    pub ship_type_id: Option<i64>,
    pub ship_name: Option<String>,
    pub ship_type: Option<String>,
}

/// One organisation in a system's threat top list.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ThreatEntity {
    pub id: i64,
    pub name: String,
    pub entity_type: String,
    pub kills: i32,
}

/// A wormhole system's threat analysis, for the threat card.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ThreatAnalysis {
    pub threat_level: crate::maps::ThreatLevel,
    pub threat_analyzed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub entities: Vec<ThreatEntity>,
}

/// A grantable subject from the access-subject search.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct AccessSubject {
    pub subject_type: crate::maps::SubjectType,
    pub subject_id: i64,
    pub name: String,
    pub ticker: Option<String>,
}

/// An API error: a status code plus a message, rendered as `{"error": "..."}`.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub fn unauthorized() -> Self {
        ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "not authenticated".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl From<MapError> for ApiError {
    fn from(err: MapError) -> Self {
        let status = match &err {
            MapError::NotFound => StatusCode::NOT_FOUND,
            MapError::Forbidden => StatusCode::FORBIDDEN,
            MapError::Conflict(_) | MapError::LastOwner => StatusCode::CONFLICT,
            MapError::Validation(_) => StatusCode::BAD_REQUEST,
            MapError::Db(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError {
            status,
            message: err.to_string(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: err.to_string(),
        }
    }
}

pub type ApiResult<T> = Result<Json<T>, ApiError>;

/// The raw session id from the cookie, if any.
fn session_id(jar: &CookieJar) -> Option<String> {
    jar.get(crate::session::SESSION_COOKIE)
        .map(|c| c.value().to_string())
}

/// The acting character from the session cookie, or `None` if not signed in.
async fn session_actor(db: &sqlx::PgPool, jar: &CookieJar) -> Result<Option<Actor>, ApiError> {
    let Some(session_id) = session_id(jar) else {
        return Ok(None);
    };
    Ok(crate::session::actor_for_session(db, &session_id).await?)
}

/// The auth guard: the acting character, or 401.
async fn require_actor(db: &sqlx::PgPool, jar: &CookieJar) -> Result<Actor, ApiError> {
    session_actor(db, jar)
        .await?
        .ok_or_else(ApiError::unauthorized)
}

/// Command bodies carry `map_id` (the action contracts authorize on it); it must agree
/// with the path so a URL can't act on a different map than it names.
fn check_map_id(path_id: i64, body_id: i64) -> Result<(), ApiError> {
    if path_id == body_id {
        Ok(())
    } else {
        Err(ApiError::bad_request("map id in body does not match URL"))
    }
}

pub fn router() -> Router<AppState> {
    use handlers as h;
    Router::new()
        .route("/api/me", get(h::me))
        .route("/api/me/status", get(h::me_status))
        .route("/api/me/characters", get(h::my_characters))
        .route("/api/me/switch-character", post(h::switch_character))
        .route("/api/me/remove-character", post(h::remove_character))
        .route("/api/waypoints", post(h::set_waypoint))
        .route("/api/waypoints/all", post(h::set_waypoint_all))
        .route("/api/grid-config", get(h::grid_config))
        .route("/api/effects", get(h::effect_modifiers))
        .route("/api/signature-types", get(h::signature_catalog))
        .route("/api/ships/search", get(h::search_ships))
        .route("/api/evescout", get(h::eve_scout))
        .route("/api/systems/search", get(h::search_systems))
        .route("/api/routing-graph", get(h::routing_graph))
        .route("/api/threat/{solar_system_id}", get(h::threat_analysis))
        .route("/api/systems/resolve", get(h::resolve_systems))
        .route("/api/maps", get(h::my_maps).post(h::create_map))
        .route("/api/maps/{id}", get(h::fetch_map).delete(h::delete_map))
        .route("/api/maps/{id}/signatures", get(h::list_signatures))
        .route("/api/maps/{id}/characters", get(h::map_characters))
        .route(
            "/api/maps/{id}/settings/user",
            get(h::map_user_settings).post(h::update_map_user_settings),
        )
        .route("/api/maps/{id}/clear", post(h::clear_map))
        .route("/api/maps/{id}/systems/add", post(h::add_system))
        .route("/api/maps/{id}/systems/move", post(h::move_systems))
        .route("/api/maps/{id}/systems/move-one", post(h::move_system))
        .route("/api/maps/{id}/systems/remove", post(h::remove_systems))
        .route("/api/maps/{id}/systems/remove-one", post(h::remove_system))
        .route("/api/maps/{id}/systems/set-alias", post(h::set_alias))
        .route("/api/maps/{id}/systems/set-status", post(h::set_status))
        .route("/api/maps/{id}/systems/set-occupier", post(h::set_occupier))
        .route("/api/maps/{id}/systems/set-home", post(h::set_home))
        .route("/api/maps/{id}/systems/set-rally", post(h::set_rally))
        .route("/api/maps/{id}/systems/set-pinned", post(h::set_pinned))
        .route("/api/maps/{id}/systems/set-notes", post(h::set_notes))
        .route(
            "/api/maps/{id}/systems/{mss}/details",
            get(h::system_details),
        )
        .route("/api/maps/{id}/connections/add", post(h::add_connection))
        .route(
            "/api/maps/{id}/connections/set-status",
            post(h::set_connection_status),
        )
        .route(
            "/api/maps/{id}/connections/remove",
            post(h::remove_connection),
        )
        .route(
            "/api/maps/{id}/connections/{cid}/jumps",
            get(h::list_connection_jumps),
        )
        .route(
            "/api/maps/{id}/connections/stale",
            get(h::list_stale_connections),
        )
        .route(
            "/api/maps/{id}/connections/clean-stale",
            post(h::clean_stale_connections),
        )
        .route(
            "/api/access-subjects/search",
            get(h::search_access_subjects),
        )
        .route("/api/maps/{id}/update", post(h::update_map))
        .route("/api/maps/{id}/access", get(h::list_access))
        .route("/api/maps/{id}/access/set", post(h::set_access))
        .route("/api/maps/{id}/access/revoke", post(h::revoke_access))
        .route("/api/maps/{id}/events", get(h::list_map_events))
        .route("/api/maps/{id}/events/undo", post(h::undo_map_event))
        .route("/api/maps/{id}/watchlist", get(h::list_watchlist))
        .route("/api/maps/{id}/watchlist/add", post(h::add_watchlist_entry))
        .route(
            "/api/maps/{id}/watchlist/set-pinned",
            post(h::set_watchlist_pinned),
        )
        .route(
            "/api/maps/{id}/watchlist/remove",
            post(h::remove_watchlist_entry),
        )
        .route(
            "/api/maps/{id}/connections/jumps/add",
            post(h::add_connection_jump),
        )
        .route(
            "/api/maps/{id}/connections/jumps/update",
            post(h::update_connection_jump),
        )
        .route(
            "/api/maps/{id}/connections/jumps/remove",
            post(h::remove_connection_jump),
        )
        .route("/api/maps/{id}/signatures/add", post(h::add_signature))
        .route("/api/maps/{id}/signatures/paste", post(h::paste_signatures))
        .route(
            "/api/maps/{id}/signatures/update",
            post(h::update_signature),
        )
        .route("/api/maps/{id}/signatures/link", post(h::link_signature))
        .route(
            "/api/maps/{id}/signatures/unlink",
            post(h::unlink_signature),
        )
        .route(
            "/api/maps/{id}/signatures/remove",
            post(h::remove_signature),
        )
        .route(
            "/api/maps/{id}/signatures/remove-bulk",
            post(h::remove_signatures_bulk),
        )
}
