//! Scan results: the signatures on each system, the paste that replaces them wholesale,
//! and the links that tie one to a connection.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};

use super::extract::{ShareQuery, acting_on, read_map_as};
use super::{ApiError, ApiResult};
use crate::auth::AppState;
use crate::maps::signatures::{
    AddSignature, LinkSignature, PasteSignatures, RemoveSignature, RemoveSignatures,
    UnlinkSignature, UpdateSignature,
};
use crate::maps::{MapEvent, Signature};

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

/// The full signature catalog, served once and cached client-side.
#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SignatureCatalog {
    pub categories: Vec<SignatureCategoryInfo>,
    pub types: Vec<SignatureTypeInfo>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/signature-types", get(signature_catalog))
        .route("/api/maps/{id}/signatures", get(list_signatures))
        .route("/api/maps/{id}/signatures/add", post(add_signature))
        .route("/api/maps/{id}/signatures/paste", post(paste_signatures))
        .route("/api/maps/{id}/signatures/update", post(update_signature))
        .route("/api/maps/{id}/signatures/link", post(link_signature))
        .route("/api/maps/{id}/signatures/unlink", post(unlink_signature))
        .route("/api/maps/{id}/signatures/remove", post(remove_signature))
        .route(
            "/api/maps/{id}/signatures/remove-bulk",
            post(remove_signatures_bulk),
        )
}

/// `GET /api/maps/{id}/signatures`, all signatures on the map.
pub async fn list_signatures(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Query(share): Query<ShareQuery>,
) -> ApiResult<Vec<Signature>> {
    read_map_as(&state, &jar, map_id, &share).await?;
    let sigs = crate::maps::signatures::read_signatures(&state.db, map_id).await?;
    Ok(Json(sigs))
}

pub async fn add_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<AddSignature>,
) -> ApiResult<Signature> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let sig = crate::maps::signatures::add_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(Json(sig))
}

pub async fn paste_signatures(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<PasteSignatures>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let solar_system_id = cmd.solar_system_id;
    crate::maps::signatures::paste_signatures(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id,
        solar_system_id,
    });
    Ok(Json(()))
}

pub async fn update_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UpdateSignature>,
) -> ApiResult<Signature> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let sig = crate::maps::signatures::update_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    if let Some(connection_id) = sig.connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id: sig.map_id,
            connection_id,
        });
    }
    Ok(Json(sig))
}

pub async fn link_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<LinkSignature>,
) -> ApiResult<Signature> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let connection_id = cmd.connection_id;
    let sig = crate::maps::signatures::link_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    state.hub.publish(MapEvent::ConnectionChanged {
        map_id: sig.map_id,
        connection_id,
    });
    Ok(Json(sig))
}

pub async fn unlink_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<UnlinkSignature>,
) -> ApiResult<Signature> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let sig = crate::maps::signatures::unlink_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(Json(sig))
}

pub async fn remove_signature(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSignature>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let outcome = crate::maps::signatures::remove_signature(&state.db, actor, cmd).await?;
    state.hub.publish(MapEvent::SignatureChanged {
        map_id,
        solar_system_id: outcome.solar_system_id,
    });
    if let Some(connection_id) = outcome.removed_connection_id {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    for map_solar_system_id in outcome.removed_placement_ids {
        state.hub.publish(MapEvent::SystemRemoved {
            map_id,
            map_solar_system_id,
        });
    }
    Ok(Json(()))
}

/// `POST /api/maps/{id}/signatures/remove-bulk`: the panel's "delete missing
/// signatures" path, with the legacy connection + orphan-endpoint cascade.
pub async fn remove_signatures_bulk(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(map_id): Path<i64>,
    Json(cmd): Json<RemoveSignatures>,
) -> ApiResult<()> {
    let actor = acting_on(&state.db, &jar, map_id, cmd.map_id).await?;
    let outcome = crate::maps::signatures::remove_signatures(&state.db, actor, cmd).await?;
    for solar_system_id in outcome.systems {
        state.hub.publish(MapEvent::SignatureChanged {
            map_id,
            solar_system_id,
        });
    }
    for connection_id in outcome.removed_connection_ids {
        state.hub.publish(MapEvent::ConnectionChanged {
            map_id,
            connection_id,
        });
    }
    for map_solar_system_id in outcome.removed_placement_ids {
        state.hub.publish(MapEvent::SystemRemoved {
            map_id,
            map_solar_system_id,
        });
    }
    Ok(Json(()))
}

/// `GET /api/signature-types`: the seeded signature catalog (categories + types with
/// spawn areas). Reference data, so no actor/role check; cacheable for a day.
pub async fn signature_catalog(
    State(state): State<AppState>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let categories = sqlx::query_as!(
        SignatureCategoryInfo,
        "select id, name, code from signature_categories order by id",
    )
    .fetch_all(&state.db)
    .await?;
    let types = sqlx::query!(
        r#"select st.id, st.signature, st.name, st.signature_category_id, st.target_class,
                  st.extra,
                  coalesce(array_agg(sa.wormhole_class_id order by sa.wormhole_class_id)
                           filter (where sa.wormhole_class_id is not null), '{}') as "spawn_areas!",
                  wt.total_mass, wt.max_mass_per_jump, wt.lifetime_hours, wt.signature_strength
           from signature_types st
           left join wormhole_types wt on wt.code = st.signature
           left join signature_type_spawn_areas sa on sa.signature_type_id = st.id
           group by st.id, wt.code
           order by st.id"#,
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| SignatureTypeInfo {
        id: r.id,
        signature: r.signature,
        name: r.name,
        signature_category_id: r.signature_category_id,
        target_class: r.target_class,
        extra: r.extra,
        spawn_areas: r.spawn_areas,
        total_mass: r.total_mass,
        max_jump_mass: r.max_mass_per_jump,
        lifetime_hours: r.lifetime_hours,
        signature_strength: r.signature_strength,
    })
    .collect();
    Ok((
        [(axum::http::header::CACHE_CONTROL, "public, max-age=86400")],
        Json(SignatureCatalog { categories, types }),
    ))
}
