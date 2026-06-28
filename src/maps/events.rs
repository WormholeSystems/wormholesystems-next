//! In-process realtime event bus for map changes.
//!
//! Mapping runs as a **single server**, so we don't need a cross-instance bus (Postgres
//! `LISTEN/NOTIFY`, Redis, …). A typed [`MapEvent`] is published to an in-process
//! [`MapHub`] — one `tokio::broadcast` channel per `map_id` — and the (future) WebSocket
//! layer subscribes per connected viewer and forwards events so the client refetches the
//! affected slice. Background producers (e.g. the tracking poller) publish to the same hub
//! directly, which is exactly the routing that `LISTEN/NOTIFY` makes awkward.
//!
//! Events are **notify-then-refetch**: a payload names *what* changed (ids), not the new
//! data. The client re-reads via the read actions; on (re)connect it does a full `get_map`,
//! so a missed event self-heals. See [`docs/features/realtime.md`](../../docs/features/realtime.md).

use serde::{Deserialize, Serialize};

// MapEvent is shared with the wasm client (it deserializes WS frames); the MapHub itself
// (tokio channels) is server-only.
#[cfg(feature = "ssr")]
use std::collections::HashMap;
#[cfg(feature = "ssr")]
use std::sync::{Arc, Mutex};

#[cfg(feature = "ssr")]
use tokio::sync::broadcast;

/// Per-map channel depth. A receiver that lags past this gets `RecvError::Lagged`; the WS
/// layer treats that as "you're behind" and triggers a full refetch.
#[cfg(feature = "ssr")]
const CHANNEL_CAPACITY: usize = 128;

/// A change to a map that its viewers should react to. Carries ids, not data — consumers
/// refetch the named slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MapEvent {
    /// The map's own fields (name / description / image) changed.
    MapUpdated { map_id: i64 },
    SystemAdded {
        map_id: i64,
        map_solar_system_id: i64,
    },
    SystemMoved {
        map_id: i64,
        map_solar_system_id: i64,
    },
    SystemRemoved {
        map_id: i64,
        map_solar_system_id: i64,
    },
    /// A connection was added, removed, or had its state changed (incl. trigger-driven sync).
    ConnectionChanged { map_id: i64, connection_id: i64 },
    /// A signature in this system was added, edited, linked, unlinked, or removed.
    SignatureChanged { map_id: i64, solar_system_id: i64 },
    /// An access grant changed (membership/roles).
    AccessChanged { map_id: i64 },
}

impl MapEvent {
    /// The map every event is routed by.
    pub fn map_id(&self) -> i64 {
        match *self {
            MapEvent::MapUpdated { map_id }
            | MapEvent::SystemAdded { map_id, .. }
            | MapEvent::SystemMoved { map_id, .. }
            | MapEvent::SystemRemoved { map_id, .. }
            | MapEvent::ConnectionChanged { map_id, .. }
            | MapEvent::SignatureChanged { map_id, .. }
            | MapEvent::AccessChanged { map_id } => map_id,
        }
    }
}

/// The in-process event bus. Cheaply cloneable (an `Arc` inside) — hold one in app state.
#[cfg(feature = "ssr")]
#[derive(Clone, Default)]
pub struct MapHub {
    channels: Arc<Mutex<HashMap<i64, broadcast::Sender<MapEvent>>>>,
}

#[cfg(feature = "ssr")]
impl MapHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe to a map's events, creating its channel on first use. Each connected
    /// viewer holds one receiver.
    pub fn subscribe(&self, map_id: i64) -> broadcast::Receiver<MapEvent> {
        let mut channels = self.channels.lock().expect("map hub poisoned");
        channels
            .entry(map_id)
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .subscribe()
    }

    /// Publish an event to its map. A no-op if nobody is watching that map; if the last
    /// subscriber has gone, the channel is pruned so idle maps don't leak.
    pub fn publish(&self, event: MapEvent) {
        let map_id = event.map_id();
        let mut channels = self.channels.lock().expect("map hub poisoned");
        if let Some(tx) = channels.get(&map_id)
            && tx.send(event).is_err()
        {
            channels.remove(&map_id);
        }
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn delivers_to_all_subscribers_of_the_map() {
        let hub = MapHub::new();
        let mut a = hub.subscribe(1);
        let mut b = hub.subscribe(1);

        hub.publish(MapEvent::AccessChanged { map_id: 1 });

        assert_eq!(a.try_recv().unwrap(), MapEvent::AccessChanged { map_id: 1 });
        assert_eq!(b.try_recv().unwrap(), MapEvent::AccessChanged { map_id: 1 });
    }

    #[tokio::test]
    async fn events_are_scoped_to_their_map() {
        let hub = MapHub::new();
        let mut other = hub.subscribe(2);

        hub.publish(MapEvent::MapUpdated { map_id: 1 });

        assert!(
            other.try_recv().is_err(),
            "map 2 must not see map 1's events"
        );
    }

    #[tokio::test]
    async fn publishing_with_no_subscribers_is_a_noop() {
        let hub = MapHub::new();
        // No panic, nothing to receive.
        hub.publish(MapEvent::SystemAdded {
            map_id: 99,
            map_solar_system_id: 1,
        });
    }
}
