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
    #[ts(optional)]
    pub description: Option<String>,
    pub role: String,
    /// How big the chain is right now.
    pub system_count: i64,
    pub connection_count: i64,
    /// How many people can see it, counting every grant however it was made.
    pub member_count: i64,
    /// Tracked pilots currently online in one of its systems, which is the difference
    /// between a map being live and merely existing.
    pub pilots_online: i64,
    /// When the chain last changed. `None` for a map nobody has touched yet.
    #[ts(optional)]
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    /// Hidden from this user's list. Per-user, so archiving does not touch anyone else.
    pub is_archived: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
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
    /// Ask which signature was jumped, rather than mapping the hole unlinked.
    pub prompt_for_signature: bool,
    /// Prefill the jump dialog's alias from the chain's naming scheme.
    pub suggest_alias: bool,
    /// Put the new connection's bookmark on the clipboard once the jump is mapped.
    pub copy_bookmark: bool,
    /// Which half of the chain the killmails card shows: `all` / `jspace` / `kspace`.
    pub killmail_filter: String,
    pub is_archived: bool,
    /// Whether this user has been through the map's introduction.
    pub introduction_confirmed: bool,
    /// Panels this user hides on this map. Empty = the built-in set. A hidden panel keeps
    /// its saved position, so unhiding puts it back where it was.
    pub hidden_panels: Vec<String>,
    /// Per-breakpoint tile positions. `None` = the built-in arrangement.
    #[ts(optional)]
    pub layout_breakpoints: Option<PanelLayouts>,
}

/// One ESI permission and whether the acting character has consented to it.
#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ScopeStatus {
    pub scope: String,
    pub granted: bool,
}

/// Tile positions keyed by breakpoint (`xs` / `sm` / `md` / `lg`).
pub type PanelLayouts = std::collections::BTreeMap<String, BreakpointLayout>;

/// One breakpoint's arrangement.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct BreakpointLayout {
    pub cols: i32,
    pub row_height: i32,
    pub items: Vec<LayoutItem>,
}

/// One tile. Minimum sizes are deliberately absent: they belong to the panel, not to
/// anyone's arrangement, so they live in the client's panel registry where tightening one
/// still reaches people who have already saved a layout.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct LayoutItem {
    /// Panel id. Named `i` to match the stored shape.
    pub i: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Panels the layout may refer to. The server keeps its own copy so a bad payload is a
/// 400 rather than a page that renders a tile nothing knows how to draw.
pub const PANEL_IDS: [&str; 9] = [
    "map",
    "navigation",
    "system-info",
    "threat",
    "signatures",
    "notes",
    "characters",
    "skyhooks",
    "killmails",
];
const BREAKPOINT_KEYS: [&str; 4] = ["xs", "sm", "md", "lg"];

/// Reject anything that would not render: unknown ids, duplicates, or a tile outside the
/// grid it claims to be in.
pub fn validate_layouts(layouts: &PanelLayouts) -> Result<(), ApiError> {
    for (key, layout) in layouts {
        if !BREAKPOINT_KEYS.contains(&key.as_str()) {
            return Err(ApiError::bad_request(format!("unknown breakpoint {key}")));
        }
        if !(1..=24).contains(&layout.cols) {
            return Err(ApiError::bad_request("cols must be between 1 and 24"));
        }
        if !(40..=400).contains(&layout.row_height) {
            return Err(ApiError::bad_request(
                "row height must be between 40 and 400",
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for item in &layout.items {
            if !PANEL_IDS.contains(&item.i.as_str()) {
                return Err(ApiError::bad_request(format!("unknown panel {}", item.i)));
            }
            if !seen.insert(item.i.as_str()) {
                return Err(ApiError::bad_request(format!("{} listed twice", item.i)));
            }
            if item.w < 1 || item.h < 1 {
                return Err(ApiError::bad_request("a tile must be at least 1x1"));
            }
            if item.x < 0 || item.y < 0 || item.x + item.w > layout.cols {
                return Err(ApiError::bad_request(format!(
                    "{} does not fit the {key} grid",
                    item.i
                )));
            }
        }
    }
    Ok(())
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
    #[serde(default)]
    #[ts(optional)]
    pub prompt_for_signature: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub suggest_alias: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub copy_bookmark: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub killmail_filter: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub is_archived: Option<bool>,
    /// Stamped server-side, so "when" is the server's clock rather than the browser's.
    #[serde(default)]
    #[ts(optional)]
    pub introduction_confirmed: Option<bool>,
    #[serde(default)]
    #[ts(optional)]
    pub hidden_panels: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub layout_breakpoints: Option<PanelLayouts>,
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

/// An online, tracked character on the map (presence), for the node pilot rows and the
/// pilots card.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapCharacter {
    pub character_id: i64,
    pub name: String,
    pub corporation_ticker: String,
    pub solar_system_id: Option<i64>,
    pub ship_type_id: Option<i64>,
    /// The name the pilot gave the hull, which is not the hull's type.
    pub ship_name: Option<String>,
    pub ship_type: Option<String>,
    /// The hull's inventory group, so the client can tell a covert-ops scanner from a
    /// combat ship without shipping a table of hull names.
    pub ship_group_id: Option<i64>,
    /// Docked in a station or structure: on the map, but not on grid.
    pub is_docked: bool,
    /// One of the viewer's own characters, so their alts can be told apart at a glance.
    pub is_mine: bool,
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

/// One hit from the map command palette. `map_solar_system_id` is set when the system is
/// already placed; otherwise the hit is an off-map system the palette can add.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct MapSearchHit {
    /// The same payload every other system picker renders, so the palette's rows line up
    /// with them instead of being styled by hand.
    pub system: SystemSearchResult,
    pub map_solar_system_id: Option<i64>,
    pub alias: Option<String>,
    pub occupying_group: Option<String>,
    /// The matching slice of the system's notes, when the query hit the notes. Member+ only.
    pub note_excerpt: Option<String>,
    /// Why this row matched: `name`, `alias`, `occupier`, or `notes`.
    pub matched: String,
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
        .route("/api/me/scopes", get(h::my_scopes))
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
        .route("/api/server-status", get(h::server_status))
        .route("/api/skyhooks", get(h::skyhooks))
        .route("/api/maps/{id}/killmails", get(h::map_killmails))
        .route("/api/maps/{id}/track-jump", post(h::track_jump))
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
        .route("/api/maps/{id}/search", get(h::search_map))
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
        .route("/api/maps/{id}/events/redo", post(h::redo_map_event))
        .route("/api/maps/{id}/events/goto", post(h::goto_map_event))
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

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn layout(cols: i32, items: Vec<LayoutItem>) -> PanelLayouts {
        PanelLayouts::from([(
            "lg".to_string(),
            BreakpointLayout {
                cols,
                row_height: 100,
                items,
            },
        )])
    }

    fn tile(i: &str, x: i32, y: i32, w: i32, h: i32) -> LayoutItem {
        LayoutItem {
            i: i.into(),
            x,
            y,
            w,
            h,
        }
    }

    #[test]
    fn accepts_a_sane_arrangement() {
        let ok = layout(10, vec![tile("map", 0, 0, 7, 9), tile("notes", 7, 0, 3, 3)]);
        assert!(validate_layouts(&ok).is_ok());
    }

    #[test]
    fn rejects_a_panel_the_client_could_not_draw() {
        // Deliberately a name no panel will ever have; naming a plausible future one
        // means the test quietly stops testing anything the day it ships.
        let bad = layout(10, vec![tile("not-a-panel", 0, 0, 2, 2)]);
        assert!(validate_layouts(&bad).is_err());
    }

    #[test]
    fn rejects_the_same_panel_twice() {
        let bad = layout(
            10,
            vec![tile("notes", 0, 0, 2, 2), tile("notes", 2, 0, 2, 2)],
        );
        assert!(validate_layouts(&bad).is_err());
    }

    #[test]
    fn rejects_a_tile_hanging_off_the_grid() {
        let bad = layout(4, vec![tile("map", 3, 0, 2, 2)]);
        assert!(validate_layouts(&bad).is_err());
    }

    #[test]
    fn rejects_an_unknown_breakpoint() {
        let bad = PanelLayouts::from([(
            "ultrawide".to_string(),
            BreakpointLayout {
                cols: 10,
                row_height: 100,
                items: vec![],
            },
        )]);
        assert!(validate_layouts(&bad).is_err());
    }

    #[test]
    fn rejects_absurd_grid_geometry() {
        assert!(validate_layouts(&layout(0, vec![])).is_err());
        assert!(validate_layouts(&layout(64, vec![])).is_err());
        let mut tall = layout(4, vec![]);
        tall.get_mut("lg").unwrap().row_height = 4000;
        assert!(validate_layouts(&tall).is_err());
    }

    #[test]
    fn rejects_a_zero_sized_tile() {
        let bad = layout(4, vec![tile("map", 0, 0, 0, 2)]);
        assert!(validate_layouts(&bad).is_err());
    }
}
