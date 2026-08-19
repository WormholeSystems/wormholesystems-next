//! The realtime WebSocket handlers: the per-map event stream and the per-user private
//! channel / activity heartbeat.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;
use tokio::sync::broadcast::error::RecvError;

use crate::auth::AppState;
use crate::maps::MapHub;
use crate::user_channel::UserHub;

/// `GET /ws/map/{map_id}` — upgrade to a WebSocket and stream the map's events as JSON.
/// Gated at the same bar as reading the map: a grant, or the share the map has been opened
/// with. The stream will eventually carry member-gated data like pilot movement, so
/// subscription must be authorized, not open.
///
/// A watcher following a share link gets the stream too. The frames say only that something
/// changed, and leaving them out would mean showing a stale chain or polling for one.
pub async fn map_ws(
    Path(map_id): Path<i64>,
    State(state): State<AppState>,
    jar: CookieJar,
    ws: WebSocketUpgrade,
) -> Response {
    let actor = match jar.get(crate::session::SESSION_COOKIE) {
        Some(cookie) => crate::session::actor_for_session(&state.db, cookie.value())
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let token = crate::api::handlers::share_cookie(&jar, map_id);

    match crate::maps::access::reader_for(&state.db, map_id, actor, token.as_deref()).await {
        Ok(_) => ws.on_upgrade(move |socket| stream_map_events(socket, state.hub, map_id)),
        Err(crate::maps::MapError::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /ws/user` — the signed-in user's private channel. For now it's the activity
/// heartbeat: while connected we ping the client and bump `last_active_at` on each pong,
/// which gates the tracking poller. User-targeted pushes will ride this same socket later.
pub async fn user_ws(
    State(state): State<AppState>,
    jar: CookieJar,
    ws: WebSocketUpgrade,
) -> Response {
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

async fn user_heartbeat(mut socket: WebSocket, pool: sqlx::PgPool, users: UserHub, user_id: i64) {
    use std::time::{Duration, Instant};

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

async fn stream_map_events(mut socket: WebSocket, hub: MapHub, map_id: i64) {
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
