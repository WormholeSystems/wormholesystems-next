//! Per-user private realtime channel — the server side of the `/ws/user` socket.
//!
//! Like [`MapHub`](crate::maps::MapHub) but keyed by `user_id` and routed explicitly: only
//! the addressed user's connections receive an event. Today it carries status-change pings
//! (the client refetches on receipt); the payload is server-only — the client just reacts to
//! a message arriving.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 64;

/// An event addressed to a single user.
#[derive(Clone, Debug, Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserEvent {
    /// One of the user's characters had its tracked status updated — refetch.
    CharacterStatusChanged { character_id: i64 },
}

/// In-process bus of per-user channels. Cheaply cloneable; hold one in app state.
#[derive(Clone, Default)]
pub struct UserHub {
    channels: Arc<Mutex<HashMap<i64, broadcast::Sender<UserEvent>>>>,
}

impl UserHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to a user's events (one per connected device), creating the channel lazily.
    pub fn subscribe(&self, user_id: i64) -> broadcast::Receiver<UserEvent> {
        let mut channels = self.channels.lock().expect("user hub poisoned");
        channels
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Send an event to a user. A no-op if they have no live connection; prunes the channel
    /// once the last one is gone.
    pub fn publish(&self, user_id: i64, event: UserEvent) {
        let mut channels = self.channels.lock().expect("user hub poisoned");
        if let Some(tx) = channels.get(&user_id)
            && tx.send(event).is_err()
        {
            channels.remove(&user_id);
        }
    }
}
