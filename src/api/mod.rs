//! The JSON HTTP API: plain Axum handlers over the [`crate::maps`] actions, plus the
//! realtime WebSocket handlers. This is the client/server boundary for the SvelteKit
//! frontend.
//!
//! One module per area, each owning its handlers, its routes and the wire types it serves.
//! [`extract`] holds the shared request plumbing, [`router`] merges the area routers.
//!
//! The acting [`Actor`](crate::maps::Actor) is resolved from the session cookie
//! server-side, never sent by the client. Each mutating handler publishes the matching
//! [`MapEvent`](crate::maps::MapEvent) to the hub after the action commits.
pub mod access;
pub mod alerts;
pub mod connections;
pub mod eve_scout;
pub mod extract;
pub mod history;
pub mod identity;
pub mod killmails;
pub mod layout;
pub mod maps;
pub mod reference;
pub mod search;
pub mod signatures;
pub mod systems;
pub mod tracking;
pub mod transfer;
pub mod user_settings;
pub mod watchlist;
pub mod ws;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::auth::AppState;
use crate::maps::MapError;

// Wire types live next to their handlers; these re-exports save the rest of the crate from
// caring which area a type belongs to.
pub use access::AccessSubject;
pub use connections::ShipSearchResult;
pub use eve_scout::EveScoutConnection;
pub use identity::{CharacterRef, CharacterStatus, CharacterSummary, ScopeStatus};
pub use layout::{BreakpointLayout, LayoutItem, PANEL_IDS, PanelLayouts, validate_layouts};
pub use maps::{MapCharacter, MapEntry};
pub use reference::{SystemSearchResult, ThreatAnalysis, ThreatEntity};
pub use search::{MapSearchHit, ThreatMatch};
pub use signatures::{SignatureCatalog, SignatureCategoryInfo, SignatureTypeInfo};
pub use user_settings::{MapUserSettings, UpdateMapUserSettings};

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

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(identity::routes())
        .merge(reference::routes())
        .merge(eve_scout::routes())
        .merge(maps::routes())
        .merge(systems::routes())
        .merge(connections::routes())
        .merge(signatures::routes())
        .merge(watchlist::routes())
        .merge(search::routes())
        .merge(access::routes())
        .merge(history::routes())
        .merge(killmails::routes())
        .merge(tracking::routes())
        .merge(user_settings::routes())
        .merge(alerts::routes())
        .merge(transfer::routes())
}
