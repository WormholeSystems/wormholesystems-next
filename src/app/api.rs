//! The server/client boundary for the map test harness: Leptos server functions wrapping
//! the [`crate::maps`] actions, plus the realtime WebSocket handler.
//!
//! Each mutating server fn publishes the matching [`MapEvent`] to the [`MapHub`] after the
//! action commits; the WS handler streams a map's events to subscribed viewers. The actor
//! is passed from the client — fine for this dev harness, but a real deployment would inject
//! it from the authenticated session instead.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::maps::Actor;
use crate::maps::connection::{AddConnection, RemoveConnection, SetConnectionStatus};
use crate::maps::signatures::{
    AddSignature, LinkSignature, RemoveSignature, UnlinkSignature, UpdateSignature,
};
use crate::maps::solar_system::{AddSystem, MoveSystem, RemoveSystem};
use crate::maps::{MapConnection, MapSolarSystem, MapView, Signature};

/// What [`seed_dev_world`] hands back: an actor to act as, and the map it owns.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DevWorld {
    pub actor: Actor,
    pub map_id: i64,
}

/// Map any action/DB error to a `ServerFnError`.
#[cfg(feature = "ssr")]
fn e<E: std::fmt::Display>(err: E) -> ServerFnError {
    ServerFnError::new(err.to_string())
}

/// Grab the request-scoped pool + event hub (provided in `main.rs`).
#[cfg(feature = "ssr")]
fn ctx() -> (sqlx::PgPool, crate::maps::MapHub) {
    (expect_context(), expect_context())
}

// --- Seed + reads ---

/// Join the **shared** dev world: a single fixed dev character + map that every browser
/// gets, so changes (and the WebSocket events for them) are visible across tabs/clients.
/// Idempotent — get-or-create, so it's safe to click repeatedly and from many sessions.
#[server(SeedDevWorldFn)]
pub async fn seed_dev_world() -> Result<DevWorld, ServerFnError> {
    let (pool, _) = ctx();
    // A fixed dev character id so everyone shares one actor (and thus one access identity).
    const DEV_CHARACTER: i64 = 1_000_000;

    let user_id: i64 = match sqlx::query_scalar("select user_id from characters where id = $1")
        .bind(DEV_CHARACTER)
        .fetch_optional(&pool)
        .await
        .map_err(e)?
    {
        Some(user_id) => user_id,
        None => {
            let user_id: i64 = sqlx::query_scalar("insert into users default values returning id")
                .fetch_one(&pool)
                .await
                .map_err(e)?;
            sqlx::query(
                "insert into corporations (id, name, ticker) values (2001, 'Dev Corp', 'DEV')
                 on conflict (id) do nothing",
            )
            .execute(&pool)
            .await
            .map_err(e)?;
            sqlx::query(
                "insert into characters (id, user_id, name, owner_hash, corporation_id)
                 values ($1, $2, 'Dev Pilot', 'dev', 2001)",
            )
            .bind(DEV_CHARACTER)
            .bind(user_id)
            .execute(&pool)
            .await
            .map_err(e)?;
            user_id
        }
    };

    let actor = Actor {
        user_id,
        character_id: DEV_CHARACTER,
    };

    // Reuse the dev character's existing map if there is one; otherwise create it.
    let map_id: i64 = match sqlx::query_scalar(
        "select map_id from map_access where subject_id = $1 and role = 'owner'
         order by map_id limit 1",
    )
    .bind(DEV_CHARACTER)
    .fetch_optional(&pool)
    .await
    .map_err(e)?
    {
        Some(map_id) => map_id,
        None => {
            crate::maps::map::create_map(
                &pool,
                actor,
                crate::maps::map::CreateMap {
                    name: "Shared Dev Map".into(),
                    description: None,
                },
            )
            .await
            .map_err(e)?
            .id
        }
    };

    Ok(DevWorld { actor, map_id })
}

#[server(FetchMapFn)]
pub async fn fetch_map(actor: Actor, map_id: i64) -> Result<MapView, ServerFnError> {
    let (pool, _) = ctx();
    crate::maps::map::get_map(&pool, actor, crate::maps::map::GetMap { map_id })
        .await
        .map_err(e)
}

#[server(ListSignaturesFn)]
pub async fn list_signatures(actor: Actor, map_id: i64) -> Result<Vec<Signature>, ServerFnError> {
    let (pool, _) = ctx();
    crate::maps::signatures::list_signatures(&pool, actor, map_id)
        .await
        .map_err(e)
}

// --- Systems ---

#[server(AddSystemFn)]
pub async fn add_system(actor: Actor, cmd: AddSystem) -> Result<MapSolarSystem, ServerFnError> {
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
pub async fn move_system(actor: Actor, cmd: MoveSystem) -> Result<(), ServerFnError> {
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
pub async fn remove_system(actor: Actor, cmd: RemoveSystem) -> Result<(), ServerFnError> {
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
pub async fn add_connection(
    actor: Actor,
    cmd: AddConnection,
) -> Result<MapConnection, ServerFnError> {
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
    actor: Actor,
    cmd: SetConnectionStatus,
) -> Result<MapConnection, ServerFnError> {
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
pub async fn remove_connection(actor: Actor, cmd: RemoveConnection) -> Result<(), ServerFnError> {
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
pub async fn add_signature(actor: Actor, cmd: AddSignature) -> Result<Signature, ServerFnError> {
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
pub async fn update_signature(
    actor: Actor,
    cmd: UpdateSignature,
) -> Result<Signature, ServerFnError> {
    let (pool, hub) = ctx();
    let sig = crate::maps::signatures::update_signature(&pool, actor, cmd)
        .await
        .map_err(e)?;
    hub.publish(crate::maps::MapEvent::SignatureChanged {
        map_id: sig.map_id,
        solar_system_id: sig.solar_system_id,
    });
    // A linked edit propagates to the connection too.
    if let Some(connection_id) = sig.connection_id {
        hub.publish(crate::maps::MapEvent::ConnectionChanged {
            map_id: sig.map_id,
            connection_id,
        });
    }
    Ok(sig)
}

#[server(LinkSignatureFn)]
pub async fn link_signature(actor: Actor, cmd: LinkSignature) -> Result<Signature, ServerFnError> {
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
pub async fn unlink_signature(
    actor: Actor,
    cmd: UnlinkSignature,
) -> Result<Signature, ServerFnError> {
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
pub async fn remove_signature(actor: Actor, cmd: RemoveSignature) -> Result<(), ServerFnError> {
    let (pool, hub) = ctx();
    let map_id = cmd.map_id;
    // Capture the system before deletion so the event can name the slice to refetch.
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
            // Behind on the channel — tell the client to do a full refetch.
            Err(RecvError::Lagged(_)) => r#"{"type":"lagged"}"#.to_string(),
            Err(RecvError::Closed) => break,
        };
        if socket.send(Message::Text(text.into())).await.is_err() {
            break; // client went away
        }
    }
}
