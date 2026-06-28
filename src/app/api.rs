//! The server/client boundary: Leptos server functions wrapping the [`crate::maps`]
//! actions, plus the realtime WebSocket handler.
//!
//! The acting [`Actor`] is resolved **from the session cookie** server-side (the auth
//! guard) — never sent by the client. Each mutating fn publishes the matching [`MapEvent`]
//! to the [`MapHub`] after the action commits; the WS handler streams a map's events to
//! subscribed viewers.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::maps::connection::{AddConnection, RemoveConnection, SetConnectionStatus};
use crate::maps::signatures::{
    AddSignature, LinkSignature, PasteSignatures, RemoveSignature, UnlinkSignature,
    UpdateSignature,
};
use crate::maps::solar_system::{
    AddSystem, ClearMap, MoveSystem, MoveSystems, RemoveSystem, RemoveSystems, SetAlias, SetHome,
    SetOccupier, SetPinned, SetRally, SetStatus,
};
use crate::maps::{MapConnection, MapSolarSystem, MapView, Signature};

/// The signed-in character, for the UI's auth state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub character_id: i64,
    pub name: String,
}

/// Live status of the active character, for the navbar readout.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterStatus {
    pub online: bool,
    pub solar_system: Option<String>,
    pub ship_type_id: Option<i64>,
    pub ship_name: Option<String>,
    pub ship_type: Option<String>,
}

/// One of the user's characters, for the switcher.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterRef {
    pub character_id: i64,
    pub name: String,
    pub is_active: bool,
}

/// A map in the user's list, with their role on it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapEntry {
    pub id: i64,
    pub name: String,
    pub role: String,
}

/// A solar system matched by the "add system" search, with just enough to display and pick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemSearchResult {
    pub id: i64,
    pub name: String,
    pub security: f64,
    pub region: String,
    pub wormhole_class_id: Option<i32>,
}

/// Map any action/DB error to a `ServerFnError`.
#[cfg(feature = "ssr")]
fn e<E: std::fmt::Display>(err: E) -> ServerFnError {
    ServerFnError::new(err.to_string())
}

// NOTE: Leptos context (`expect_context`) lives in a thread-local owner, which is lost when
// a future resumes on another worker after an `.await`. So every server fn grabs `pool()` /
// `hub()` **synchronously at the top, before any await**, and passes the pool into helpers.

/// The DB pool from the request context. Call before any `.await`.
#[cfg(feature = "ssr")]
fn pool() -> sqlx::PgPool {
    expect_context()
}

/// The event hub from the request context. Call before any `.await`.
#[cfg(feature = "ssr")]
fn hub() -> crate::maps::MapHub {
    expect_context()
}

/// The raw session id from the cookie, if any.
#[cfg(feature = "ssr")]
async fn session_cookie() -> Result<Option<String>, ServerFnError> {
    use axum_extra::extract::CookieJar;
    let jar = leptos_axum::extract::<CookieJar>()
        .await
        .map_err(|err| ServerFnError::new(format!("no request context: {err}")))?;
    Ok(jar
        .get(crate::session::SESSION_COOKIE)
        .map(|c| c.value().to_string()))
}

/// The acting character from the session cookie, or `None` if not signed in.
#[cfg(feature = "ssr")]
async fn session_actor(pool: &sqlx::PgPool) -> Result<Option<crate::maps::Actor>, ServerFnError> {
    let Some(session_id) = session_cookie().await? else {
        return Ok(None);
    };
    crate::session::actor_for_session(pool, &session_id)
        .await
        .map_err(e)
}

/// The auth guard: the acting character, or a `not authenticated` error.
#[cfg(feature = "ssr")]
async fn require_actor(pool: &sqlx::PgPool) -> Result<crate::maps::Actor, ServerFnError> {
    session_actor(pool)
        .await?
        .ok_or_else(|| ServerFnError::new("not authenticated"))
}

// --- Auth / identity ---

/// Who's signed in, if anyone — drives the auth-gated UI.
#[server(CurrentCharacterFn)]
pub async fn current_character() -> Result<Option<CharacterSummary>, ServerFnError> {
    let pool = pool();
    let Some(actor) = session_actor(&pool).await? else {
        return Ok(None);
    };
    let row = sqlx::query!(
        "select name from characters where id = $1",
        actor.character_id
    )
    .fetch_optional(&pool)
    .await
    .map_err(e)?;
    Ok(row.map(|r| CharacterSummary {
        character_id: actor.character_id,
        name: r.name,
    }))
}

/// Live status of the signed-in active character (online / system / ship), for the navbar.
#[server(ActiveCharacterStatusFn)]
pub async fn active_character_status() -> Result<Option<CharacterStatus>, ServerFnError> {
    let pool = pool();
    let Some(actor) = session_actor(&pool).await? else {
        return Ok(None);
    };
    let row = sqlx::query!(
        r#"select s.online,
                  s.ship_type_id,
                  ss.name as "solar_system?",
                  s.ship_name,
                  t.name as "ship_type?"
           from character_status s
           left join solar_systems ss on ss.id = s.solar_system_id
           left join types t on t.id = s.ship_type_id
           where s.character_id = $1"#,
        actor.character_id,
    )
    .fetch_optional(&pool)
    .await
    .map_err(e)?;
    Ok(row.map(|r| CharacterStatus {
        online: r.online,
        solar_system: r.solar_system,
        ship_type_id: r.ship_type_id,
        ship_name: r.ship_name,
        ship_type: r.ship_type,
    }))
}

/// The user's characters, marking the active one (for the switcher).
#[server(MyCharactersFn)]
pub async fn my_characters() -> Result<Vec<CharacterRef>, ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    let rows = sqlx::query!(
        "select id, name from characters where user_id = $1 order by name",
        actor.user_id,
    )
    .fetch_all(&pool)
    .await
    .map_err(e)?;
    Ok(rows
        .into_iter()
        .map(|r| CharacterRef {
            character_id: r.id,
            name: r.name,
            is_active: r.id == actor.character_id,
        })
        .collect())
}

/// Switch the session's active character (must belong to the user).
#[server(SwitchCharacterFn)]
pub async fn switch_character(character_id: i64) -> Result<(), ServerFnError> {
    let pool = pool();
    let Some(session_id) = session_cookie().await? else {
        return Err(ServerFnError::new("not authenticated"));
    };
    let ok = crate::session::set_active_character(&pool, &session_id, character_id)
        .await
        .map_err(e)?;
    if !ok {
        return Err(ServerFnError::new("that character isn't yours"));
    }
    Ok(())
}

/// Remove one of the user's characters. Refuses to remove the last one. If it's the active
/// character, the session switches to another first (so the session isn't cascade-deleted).
#[server(RemoveCharacterFn)]
pub async fn remove_character(character_id: i64) -> Result<(), ServerFnError> {
    let pool = pool();
    let Some(session_id) = session_cookie().await? else {
        return Err(ServerFnError::new("not authenticated"));
    };
    let actor = require_actor(&pool).await?;

    let count = sqlx::query_scalar!(
        "select count(*) from characters where user_id = $1",
        actor.user_id,
    )
    .fetch_one(&pool)
    .await
    .map_err(e)?
    .unwrap_or(0);
    if count <= 1 {
        return Err(ServerFnError::new("can't remove your only character"));
    }

    let owns = sqlx::query_scalar!(
        "select exists(select 1 from characters where id = $1 and user_id = $2)",
        character_id,
        actor.user_id,
    )
    .fetch_one(&pool)
    .await
    .map_err(e)?
    .unwrap_or(false);
    if !owns {
        return Err(ServerFnError::new("that character isn't yours"));
    }

    // Switch away before deleting the active one (sessions FK-cascade on the character).
    if character_id == actor.character_id
        && let Some(other) = sqlx::query_scalar!(
            "select id from characters where user_id = $1 and id <> $2
             order by is_preferred desc, id limit 1",
            actor.user_id,
            character_id,
        )
        .fetch_optional(&pool)
        .await
        .map_err(e)?
    {
        crate::session::set_active_character(&pool, &session_id, other)
            .await
            .map_err(e)?;
    }

    sqlx::query!(
        "delete from characters where id = $1 and user_id = $2",
        character_id,
        actor.user_id,
    )
    .execute(&pool)
    .await
    .map_err(e)?;
    Ok(())
}

// --- Maps list ---

/// Every map the signed-in character can access, with their role.
#[server(MyMapsFn)]
pub async fn my_maps() -> Result<Vec<MapEntry>, ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    let maps = crate::maps::map::list_maps(&pool, actor.user_id)
        .await
        .map_err(e)?;
    Ok(maps
        .into_iter()
        .map(|(m, role)| MapEntry {
            id: m.id,
            name: m.name,
            role: role.as_str().to_string(),
        })
        .collect())
}

/// Create a map owned by the active character. Returns its id.
#[server(CreateMapFn)]
pub async fn create_map(name: String) -> Result<i64, ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    let map = crate::maps::map::create_map(
        &pool,
        actor,
        crate::maps::map::CreateMap {
            name,
            description: None,
        },
    )
    .await
    .map_err(e)?;
    Ok(map.id)
}

/// Delete a map (owner only).
#[server(DeleteMapFn)]
pub async fn delete_map(map_id: i64) -> Result<(), ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    crate::maps::map::delete_map(&pool, actor, crate::maps::map::DeleteMap { map_id })
        .await
        .map_err(e)?;
    Ok(())
}

// --- Reads ---

#[server(FetchMapFn)]
pub async fn fetch_map(map_id: i64) -> Result<MapView, ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    crate::maps::map::get_map(&pool, actor, crate::maps::map::GetMap { map_id })
        .await
        .map_err(e)
}

#[server(ListSignaturesFn)]
pub async fn list_signatures(map_id: i64) -> Result<Vec<Signature>, ServerFnError> {
    let pool = pool();
    let actor = require_actor(&pool).await?;
    crate::maps::signatures::list_signatures(&pool, actor, map_id)
        .await
        .map_err(e)
}

// --- Systems ---

/// Search the SDE solar systems by name for the "add system" picker. Prefix matches rank
/// first, then shorter names, then alphabetical. Returns nothing for queries under 2 chars.
#[server(SearchSystemsFn)]
pub async fn search_systems(query: String) -> Result<Vec<SystemSearchResult>, ServerFnError> {
    let pool = pool();
    require_actor(&pool).await?;
    let q = query.trim();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let contains = format!("%{q}%");
    let prefix = format!("{q}%");
    sqlx::query_as!(
        SystemSearchResult,
        r#"
        select s.id,
               s.name,
               s.security_status as "security!",
               r.name            as "region!",
               s.wormhole_class_id
        from solar_systems s
        join regions r on r.id = s.region_id
        where s.name ilike $1
        order by (s.name ilike $2) desc, length(s.name), s.name
        limit 30
        "#,
        contains,
        prefix,
    )
    .fetch_all(&pool)
    .await
    .map_err(e)
}

#[server(AddSystemFn)]
pub async fn add_system(cmd: AddSystem) -> Result<MapSolarSystem, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    let placed = crate::maps::solar_system::add_system(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemAdded {
        map_id,
        map_solar_system_id: placed.id,
    });
    Ok(placed)
}

#[server(MoveSystemFn)]
pub async fn move_system(cmd: MoveSystem) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::move_system(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemMoved {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(MoveSystemsFn)]
pub async fn move_systems(cmd: MoveSystems) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    crate::maps::solar_system::move_systems(&pool, actor, cmd)
        .await
        .map_err(e)?;
    // Several systems moved at once → one coarse event rather than N SystemMoved events.
    hub.publish(crate::maps::MapEvent::MapUpdated { map_id });
    Ok(())
}

#[server(RemoveSystemFn)]
pub async fn remove_system(cmd: RemoveSystem) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::remove_system(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemRemoved {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

// Bulk removal (multi-select delete + clear map). Publishes a single coarse `MapUpdated`
// so each viewer does one full refetch rather than reacting to a storm of per-system events.

#[server(RemoveSystemsFn)]
pub async fn remove_systems(cmd: RemoveSystems) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    crate::maps::solar_system::remove_systems(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::MapUpdated { map_id });
    Ok(())
}

#[server(ClearMapFn)]
pub async fn clear_map(cmd: ClearMap) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    crate::maps::solar_system::clear_map(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::MapUpdated { map_id });
    Ok(())
}

// --- System details (alias / status / occupier / home / pinned) ---
//
// All five publish `SystemDetailsChanged` (position unchanged; details refetched).

#[server(SetAliasFn)]
pub async fn set_alias(cmd: SetAlias) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_alias(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(SetStatusFn)]
pub async fn set_status(cmd: SetStatus) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_status(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(SetOccupierFn)]
pub async fn set_occupier(cmd: SetOccupier) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_occupier(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(SetHomeFn)]
pub async fn set_home(cmd: SetHome) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_home(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(SetRallyFn)]
pub async fn set_rally(cmd: SetRally) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_rally(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

#[server(SetPinnedFn)]
pub async fn set_pinned(cmd: SetPinned) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, mss) = (cmd.map_id, cmd.map_solar_system_id);
    crate::maps::solar_system::set_pinned(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SystemDetailsChanged {
        map_id,
        map_solar_system_id: mss,
    });
    Ok(())
}

// --- Config ---

/// The server-owned map canvas geometry (cell size + world/viewport dimensions). Static, so
/// the client fetches it once; provided into context alongside the pool/hub.
#[server(GridConfigFn)]
pub async fn grid_config() -> Result<crate::app::GridConfig, ServerFnError> {
    Ok(leptos::prelude::expect_context::<crate::app::GridConfig>())
}

/// The buffs/debuffs a wormhole effect applies at a system's class — for the node's effect
/// popover. Reference data, so no actor/role check.
#[server(EffectModifiersFn)]
pub async fn effect_modifiers(
    effect_name: String,
    wormhole_class_id: i32,
) -> Result<Vec<crate::maps::EffectModifier>, ServerFnError> {
    let pool = pool();
    crate::maps::solar_system::effect_modifiers(&pool, &effect_name, wormhole_class_id)
        .await
        .map_err(e)
}

// --- Connections ---

#[server(AddConnectionFn)]
pub async fn add_connection(cmd: AddConnection) -> Result<MapConnection, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    let conn = crate::maps::connection::add_connection(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::ConnectionChanged {
        map_id,
        connection_id: conn.id,
    });
    Ok(conn)
}

#[server(SetConnectionStatusFn)]
pub async fn set_connection_status(
    cmd: SetConnectionStatus,
) -> Result<MapConnection, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, connection_id) = (cmd.map_id, cmd.connection_id);
    let conn = crate::maps::connection::set_connection_status(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::ConnectionChanged {
        map_id,
        connection_id,
    });
    Ok(conn)
}

#[server(RemoveConnectionFn)]
pub async fn remove_connection(cmd: RemoveConnection) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, connection_id) = (cmd.map_id, cmd.connection_id);
    crate::maps::connection::remove_connection(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::ConnectionChanged {
        map_id,
        connection_id,
    });
    Ok(())
}

// --- Signatures ---

#[server(AddSignatureFn)]
pub async fn add_signature(cmd: AddSignature) -> Result<Signature, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let sig = crate::maps::signatures::add_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(sig)
}

#[server(PasteSignaturesFn)]
pub async fn paste_signatures(cmd: PasteSignatures) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let (map_id, solar_system_id) = (cmd.map_id, cmd.solar_system_id);
    crate::maps::signatures::paste_signatures(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id,
        solar_system_id,
    });
    Ok(())
}

#[server(UpdateSignatureFn)]
pub async fn update_signature(cmd: UpdateSignature) -> Result<Signature, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let sig = crate::maps::signatures::update_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    if let Some(connection_id) = sig.connection_id {
        hub.publish(crate::maps::MapEvent::ConnectionChanged {
            map_id: sig.map_id,
            connection_id,
        });
    }
    Ok(sig)
}

#[server(LinkSignatureFn)]
pub async fn link_signature(cmd: LinkSignature) -> Result<Signature, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let connection_id = cmd.connection_id;
    let sig = crate::maps::signatures::link_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    hub.publish(crate::maps::MapEvent::ConnectionChanged {
        map_id: sig.map_id,
        connection_id,
    });
    Ok(sig)
}

#[server(UnlinkSignatureFn)]
pub async fn unlink_signature(cmd: UnlinkSignature) -> Result<Signature, ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let sig = crate::maps::signatures::unlink_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(sig)
}

#[server(RemoveSignatureFn)]
pub async fn remove_signature(cmd: RemoveSignature) -> Result<(), ServerFnError> {
    let pool = pool();
    let hub = hub();
    let actor = require_actor(&pool).await?;
    let map_id = cmd.map_id;
    let solar_system_id: Option<i64> =
        sqlx::query_scalar("select solar_system_id from signatures where id = $1 and map_id = $2")
            .bind(cmd.signature_pk)
            .bind(map_id)
            .fetch_optional(&pool)
            .await
            .map_err(e)?;
    crate::maps::signatures::remove_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    if let Some(solar_system_id) = solar_system_id {
        hub.publish(crate::maps::MapEvent::SignatureChanged {
            map_id,
            solar_system_id,
        });
    }
    Ok(())
}

// --- Realtime WebSocket ---

/// `GET /ws/map/{map_id}` — upgrade to a WebSocket and stream the map's events as JSON.
/// Gated: the caller must have a valid session and at least Viewer access to the map (the
/// same bar as reading it). The stream will eventually carry member-gated data like pilot
/// movement, so subscription must be authorized, not open.
#[cfg(feature = "ssr")]
pub async fn map_ws(
    axum::extract::Path(map_id): axum::extract::Path<i64>,
    axum::extract::State(state): axum::extract::State<crate::auth::AppState>,
    jar: axum_extra::extract::CookieJar,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let actor = match jar.get(crate::session::SESSION_COOKIE) {
        Some(cookie) => crate::session::actor_for_session(&state.db, cookie.value())
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let Some(actor) = actor else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    // Any role (Viewer+) may watch a map's changes; finer per-event gating (e.g. member-only
    // pilot movement) will filter inside the stream once those events exist.
    match crate::maps::access::effective_role(&state.db, map_id, actor.user_id).await {
        Ok(Some(_)) => ws.on_upgrade(move |socket| stream_map_events(socket, state.hub, map_id)),
        Ok(None) => StatusCode::FORBIDDEN.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /ws/user` — the signed-in user's private channel. For now it's the activity
/// heartbeat: while connected we ping the client and bump `last_active_at` on each pong,
/// which gates the tracking poller. User-targeted pushes will ride this same socket later.
#[cfg(feature = "ssr")]
pub async fn user_ws(
    axum::extract::State(state): axum::extract::State<crate::auth::AppState>,
    jar: axum_extra::extract::CookieJar,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let actor = match jar.get(crate::session::SESSION_COOKIE) {
        Some(cookie) => crate::session::actor_for_session(&state.db, cookie.value())
            .await
            .ok()
            .flatten(),
        None => None,
    };
    match actor {
        Some(actor) => ws.on_upgrade(move |socket| {
            user_heartbeat(socket, state.db, state.user_hub, actor.user_id)
        }),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

#[cfg(feature = "ssr")]
async fn user_heartbeat(
    mut socket: axum::extract::ws::WebSocket,
    pool: sqlx::PgPool,
    users: crate::user_channel::UserHub,
    user_id: i64,
) {
    use std::time::{Duration, Instant};

    use axum::extract::ws::Message;
    use tokio::sync::broadcast::error::RecvError;
    use tokio::time::{MissedTickBehavior, interval};

    // Active on connect, then again on each heartbeat (throttled) while the socket lives.
    let _ = crate::session::touch_activity(&pool, user_id).await;
    let mut ping = interval(Duration::from_secs(30));
    ping.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_bump = Instant::now();
    let mut events = users.subscribe(user_id);

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break; // client gone
                }
            }
            frame = socket.recv() => {
                match frame {
                    // Any frame (pong / message) means the client is alive.
                    Some(Ok(_)) => {
                        if last_bump.elapsed() >= Duration::from_secs(50) {
                            let _ = crate::session::touch_activity(&pool, user_id).await;
                            last_bump = Instant::now();
                        }
                    }
                    _ => break, // closed or errored
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        let json = serde_json::to_string(&event).unwrap_or_default();
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => {} // client refetches regardless
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
}

#[cfg(feature = "ssr")]
async fn stream_map_events(
    mut socket: axum::extract::ws::WebSocket,
    hub: crate::maps::MapHub,
    map_id: i64,
) {
    use axum::extract::ws::Message;
    use tokio::sync::broadcast::error::RecvError;

    let mut rx = hub.subscribe(map_id);
    loop {
        let text = match rx.recv().await {
            Ok(event) => serde_json::to_string(&event).unwrap_or_default(),
            Err(RecvError::Lagged(_)) => r#"{"type":"lagged"}"#.to_string(),
            Err(RecvError::Closed) => break,
        };
        if socket.send(Message::Text(text.into())).await.is_err() {
            break;
        }
    }
}
