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
    AddSignature, LinkSignature, RemoveSignature, UnlinkSignature, UpdateSignature,
};
use crate::maps::solar_system::{AddSystem, MoveSystem, RemoveSystem};
use crate::maps::{MapConnection, MapSolarSystem, MapView, Signature};

/// The signed-in character, for the UI's auth state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub character_id: i64,
    pub name: String,
}

/// Map any action/DB error to a `ServerFnError`.
#[cfg(feature = "ssr")]
fn e<E: std::fmt::Display>(err: E) -> ServerFnError {
    ServerFnError::new(err.to_string())
}

/// The DB pool + event hub from the request context (provided in `main.rs`).
#[cfg(feature = "ssr")]
fn ctx() -> (sqlx::PgPool, crate::maps::MapHub) {
    (expect_context(), expect_context())
}

/// The acting character from the session cookie, or `None` if not signed in.
#[cfg(feature = "ssr")]
async fn session_actor() -> Result<Option<crate::maps::Actor>, ServerFnError> {
    use axum_extra::extract::CookieJar;
    let jar = leptos_axum::extract::<CookieJar>()
        .await
        .map_err(|err| ServerFnError::new(format!("no request context: {err}")))?;
    let Some(cookie) = jar.get(crate::session::SESSION_COOKIE) else {
        return Ok(None);
    };
    let pool = expect_context::<sqlx::PgPool>();
    crate::session::actor_for_session(&pool, cookie.value())
        .await
        .map_err(e)
}

/// The auth guard: the acting character, or a `not authenticated` error.
#[cfg(feature = "ssr")]
async fn require_actor() -> Result<crate::maps::Actor, ServerFnError> {
    session_actor()
        .await?
        .ok_or_else(|| ServerFnError::new("not authenticated"))
}

// --- Auth / identity ---

/// Who's signed in, if anyone — drives the auth-gated UI.
#[server(CurrentCharacterFn)]
pub async fn current_character() -> Result<Option<CharacterSummary>, ServerFnError> {
    let Some(actor) = session_actor().await? else {
        return Ok(None);
    };
    let pool = expect_context::<sqlx::PgPool>();
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

// --- Reads ---

#[server(FetchMapFn)]
pub async fn fetch_map(map_id: i64) -> Result<MapView, ServerFnError> {
    let actor = require_actor().await?;
    let (pool, _) = ctx();
    crate::maps::map::get_map(&pool, actor, crate::maps::map::GetMap { map_id })
        .await
        .map_err(e)
}

#[server(ListSignaturesFn)]
pub async fn list_signatures(map_id: i64) -> Result<Vec<Signature>, ServerFnError> {
    let actor = require_actor().await?;
    let (pool, _) = ctx();
    crate::maps::signatures::list_signatures(&pool, actor, map_id)
        .await
        .map_err(e)
}

// --- Systems ---

#[server(AddSystemFn)]
pub async fn add_system(cmd: AddSystem) -> Result<MapSolarSystem, ServerFnError> {
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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

#[server(RemoveSystemFn)]
pub async fn remove_system(cmd: RemoveSystem) -> Result<(), ServerFnError> {
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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

// --- Connections ---

#[server(AddConnectionFn)]
pub async fn add_connection(cmd: AddConnection) -> Result<MapConnection, ServerFnError> {
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
    let sig = crate::maps::signatures::add_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    Ok(sig)
}

#[server(UpdateSignatureFn)]
pub async fn update_signature(cmd: UpdateSignature) -> Result<Signature, ServerFnError> {
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
    let actor = require_actor().await?;
    let (pool, hub) = ctx();
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
#[cfg(feature = "ssr")]
pub async fn map_ws(
    axum::extract::Path(map_id): axum::extract::Path<i64>,
    axum::extract::State(state): axum::extract::State<crate::auth::AppState>,
    ws: axum::extract::ws::WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| stream_map_events(socket, state.hub, map_id))
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
