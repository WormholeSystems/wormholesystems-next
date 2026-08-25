//! Map import and export in the legacy-compatible file format: downloading a map as a
//! JSON file, merging a file into an existing map, and creating a new map from one.
//!
//! The file travels as text inside a JSON body rather than as a multipart upload: the
//! client already has it in memory (it peeks inside to offer the section choices), and the
//! server validates the text the same way either way.

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use super::extract::require_actor;
use super::{ApiError, ApiResult};
use crate::auth::AppState;
use crate::maps::transfer::{
    ImportSummary, SectionSet, TransferCounts, export_map, import_map, import_map_as_new,
    parse_export, transfer_counts,
};

/// Alliance-scale exports run to a few megabytes; comfortably above that, and still a
/// ceiling.
const IMPORT_BODY_LIMIT: usize = 32 * 1024 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/maps/{id}/transfer/counts", get(counts))
        .route("/api/maps/{id}/transfer/export", get(export))
        .route("/api/maps/{id}/transfer/import", post(import))
        .route("/api/maps/transfer/import-new", post(import_new))
        .layer(DefaultBodyLimit::max(IMPORT_BODY_LIMIT))
}

/// `GET /api/maps/{id}/transfer/counts`: how much of the map each section carries.
/// Manager+.
async fn counts(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
) -> ApiResult<TransferCounts> {
    let actor = require_actor(&state.db, &jar).await?;
    Ok(Json(transfer_counts(&state.db, actor, map_id).await?))
}

#[derive(Deserialize)]
struct ExportQuery {
    /// Comma-separated section names.
    sections: String,
}

/// `GET /api/maps/{id}/transfer/export?sections=...`: the selected sections as a JSON file
/// download. Manager+. A GET with a `content-disposition`, so the browser does the saving.
async fn export(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let actor = require_actor(&state.db, &jar).await?;
    let sections = SectionSet::from_names(
        &query
            .sections
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>(),
    )?;
    let payload = export_map(&state.db, actor, map_id, sections).await?;

    let filename = format!(
        "{}-export-{}.json",
        slug(&payload.map_name),
        payload.exported_at.format("%Y-%m-%d"),
    );
    let body = serde_json::to_string_pretty(&payload).map_err(|e| ApiError {
        status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        message: e.to_string(),
    })?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response())
}

/// The map name as a filename: lowercase, runs of anything else collapsed to one dash.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "map".to_string()
    } else {
        trimmed.to_string()
    }
}

#[derive(Deserialize)]
struct ImportBody {
    sections: Vec<String>,
    /// The uploaded file, verbatim.
    content: String,
}

/// `POST /api/maps/{id}/transfer/import`: merge a file's selected sections into the map.
/// Manager+.
async fn import(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    body: Result<Json<ImportBody>, JsonRejection>,
) -> ApiResult<ImportSummary> {
    let Json(body) = body.map_err(|e| ApiError::bad_request(e.to_string()))?;
    let actor = require_actor(&state.db, &jar).await?;
    let sections = SectionSet::from_names(&body.sections)?;
    let parsed = parse_export(&body.content, sections, false)?;
    Ok(Json(import_map(&state.db, actor, map_id, &parsed).await?))
}

#[derive(Deserialize)]
struct ImportNewBody {
    /// Overrides the file's map name when present.
    #[serde(default)]
    name: Option<String>,
    sections: Vec<String>,
    content: String,
}

/// `POST /api/maps/transfer/import-new`: create a fresh map from a file, owned by the
/// acting character.
async fn import_new(
    State(state): State<AppState>,
    jar: CookieJar,
    body: Result<Json<ImportNewBody>, JsonRejection>,
) -> ApiResult<crate::maps::Map> {
    let Json(body) = body.map_err(|e| ApiError::bad_request(e.to_string()))?;
    let actor = require_actor(&state.db, &jar).await?;
    let sections = SectionSet::from_names(&body.sections)?;
    let parsed = parse_export(&body.content, sections, true)?;
    Ok(Json(
        import_map_as_new(&state.db, actor, parsed, body.name).await?,
    ))
}
