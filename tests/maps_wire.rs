//! The server<->client wire contract. Server functions pass these types across the
//! ssr/wasm boundary, so they must serde round-trip — including the partial-update
//! `Option<Option<_>>` fields and the tagged `MapEvent` representation the WS client parses.

use chrono::{DateTime, Utc};

use vector::maps::connection::SetConnectionStatus;
use vector::maps::signatures::AddSignature;
use vector::maps::{
    ConnectionType, Map, MapConnection, MapEvent, MapSolarSystem, MapView, MassStatus,
    SignatureGroup, TimeStatus, WormholeSize,
};

/// A fixed timestamp (chrono's `clock` feature is off — no `Utc::now()`).
fn ts() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap()
}

/// Serialize → deserialize → re-serialize. The two JSON strings must match (lets us assert a
/// faithful round-trip for types that don't derive `PartialEq`).
fn reserialize<T: serde::Serialize + serde::de::DeserializeOwned>(value: &T) -> (String, String) {
    let first = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&first).unwrap();
    (first.clone(), serde_json::to_string(&back).unwrap())
}

#[test]
fn map_event_round_trips_and_reports_its_map() {
    let events = [
        MapEvent::MapUpdated { map_id: 1 },
        MapEvent::SystemAdded {
            map_id: 2,
            map_solar_system_id: 9,
        },
        MapEvent::SystemMoved {
            map_id: 2,
            map_solar_system_id: 9,
        },
        MapEvent::SystemRemoved {
            map_id: 2,
            map_solar_system_id: 9,
        },
        MapEvent::ConnectionChanged {
            map_id: 3,
            connection_id: 7,
        },
        MapEvent::SignatureChanged {
            map_id: 4,
            solar_system_id: 30000142,
        },
        MapEvent::AccessChanged { map_id: 5 },
    ];
    for ev in events {
        let json = serde_json::to_string(&ev).unwrap();
        let back: MapEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back, "round-trip changed the event");
        assert_eq!(ev.map_id(), back.map_id());
    }
}

#[test]
fn map_event_is_tagged_snake_case() {
    // The WS client matches on this shape.
    let json = serde_json::to_string(&MapEvent::SystemAdded {
        map_id: 1,
        map_solar_system_id: 2,
    })
    .unwrap();
    assert!(json.contains(r#""type":"system_added""#), "got {json}");
    assert!(json.contains(r#""map_solar_system_id":2"#), "got {json}");
}

#[test]
fn partial_update_command_preserves_leave_vs_clear_vs_set() {
    // Some(Some(v)) = set, Some(None) = clear, None = leave — all must survive the wire.
    let cmd = SetConnectionStatus {
        map_id: 1,
        connection_id: 2,
        mass_status: Some(Some(MassStatus::Critical)),
        time_status: Some(None),
        size: None,
    };
    let (a, b) = reserialize(&cmd);
    assert_eq!(a, b);
    assert!(a.contains(r#""mass_status":"critical""#), "got {a}");
    assert!(
        a.contains(r#""time_status":null"#),
        "clear must serialize as null: {a}"
    );
}

#[test]
fn add_signature_command_round_trips() {
    let cmd = AddSignature {
        map_id: 1,
        solar_system_id: 30000142,
        signature_id: "ABC-123".into(),
        group: SignatureGroup::Wormhole,
        name: Some("C247".into()),
        size: Some(WormholeSize::Large),
        mass_status: Some(MassStatus::Reduced),
        time_status: Some(TimeStatus::Eol),
    };
    let (a, b) = reserialize(&cmd);
    assert_eq!(a, b);
}

#[test]
fn map_view_round_trips() {
    let view = MapView {
        map: Map {
            id: 1,
            name: "Dev".into(),
            description: None,
            image_url: None,
            created_at: ts(),
        },
        systems: vec![MapSolarSystem {
            id: 10,
            map_id: 1,
            solar_system_id: 30000142,
            position_x: 1.5,
            position_y: -2.0,
            alias: Some("Jita".into()),
            created_at: ts(),
        }],
        connections: vec![MapConnection {
            id: 20,
            map_id: 1,
            from_system: 10,
            to_system: 11,
            kind: ConnectionType::Wormhole,
            mass_status: Some(MassStatus::Critical),
            time_status: Some(TimeStatus::Eol),
            size: None,
            created_at: ts(),
            updated_at: ts(),
        }],
    };
    let (a, b) = reserialize(&view);
    assert_eq!(a, b);
    assert!(a.contains(r#""kind":"wormhole""#), "got {a}");
}
