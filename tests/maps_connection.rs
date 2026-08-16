//! Connections: connecting and disconnecting placed systems, and marking life-cycle state.

mod common;

use common::{SYS_A, SYS_B, member_with_role, world};
use sqlx::PgPool;
use vector::maps::connection::{
    AddConnection, RemoveConnection, SetConnectionStatus, add_connection, remove_connection,
    set_connection_status,
};
use vector::maps::solar_system::{AddSystem, add_system};
use vector::maps::{ConnectionType, MapError, MassStatus, Role, TimeStatus};

#[sqlx::test]
async fn add_connection_returns_fields_and_validates(pool: PgPool) {
    let w = world(&pool).await;
    let a = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            x: 0.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();
    let b = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_B,
            x: 1.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();

    // Self-connection rejected (pure validation).
    assert!(matches!(
        add_connection(
            &pool,
            w.owner,
            AddConnection {
                map_id: w.map_id,
                from_system: a.id,
                to_system: a.id,
                kind: ConnectionType::Wormhole,
                size: None,
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));
    // Endpoint not on the map rejected.
    assert!(matches!(
        add_connection(
            &pool,
            w.owner,
            AddConnection {
                map_id: w.map_id,
                from_system: a.id,
                to_system: 4242,
                kind: ConnectionType::Wormhole,
                size: None,
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a.id,
            to_system: b.id,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(conn.from_system, a.id);
    assert_eq!(conn.to_system, b.id);
    assert_eq!(conn.kind, ConnectionType::Wormhole);

    // The same pair may be connected again (parallel edges are allowed): a second,
    // distinct connection is created — even reversed and of a different kind.
    let dup = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: b.id,
            to_system: a.id,
            kind: ConnectionType::Stargate,
            size: None,
        },
    )
    .await
    .unwrap();
    assert_ne!(dup.id, conn.id);
    let edges: i64 = sqlx::query_scalar("select count(*) from map_connections where map_id = $1")
        .bind(w.map_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(edges, 2);
}

#[sqlx::test]
async fn remove_connection_clears_signature_link(pool: PgPool) {
    let w = world(&pool).await;
    let a = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            x: 0.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();
    let b = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_B,
            x: 1.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a.id,
            to_system: b.id,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"insert into signatures (map_id, solar_system_id, signature_id, "group", connection_id)
           values ($1, $2, 'ABC-123', 'wormhole', $3)"#,
    )
    .bind(w.map_id)
    .bind(SYS_A)
    .bind(conn.id)
    .execute(&pool)
    .await
    .unwrap();

    remove_connection(
        &pool,
        w.owner,
        RemoveConnection {
            map_id: w.map_id,
            connection_id: conn.id,
        },
    )
    .await
    .unwrap();

    // The signature survives, with its connection_id cleared.
    let total: i64 = sqlx::query_scalar("select count(*) from signatures where map_id = $1")
        .bind(w.map_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let linked: i64 =
        sqlx::query_scalar("select count(*) from signatures where connection_id is not null")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(total, 1);
    assert_eq!(linked, 0);

    // Unknown connection → NotFound.
    assert!(matches!(
        remove_connection(
            &pool,
            w.owner,
            RemoveConnection {
                map_id: w.map_id,
                connection_id: 4242
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}

#[sqlx::test]
async fn set_status_marks_a_connection_without_a_signature(pool: PgPool) {
    let w = world(&pool).await;
    let a = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            x: 0.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();
    let b = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_B,
            x: 1.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap();
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a.id,
            to_system: b.id,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        conn.mass_status, None,
        "new connections start with unknown state"
    );

    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;
    assert!(matches!(
        set_connection_status(
            &pool,
            viewer,
            SetConnectionStatus {
                map_id: w.map_id,
                connection_id: conn.id,
                mass_status: Some(Some(MassStatus::Critical)),
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    // Mark mass + EOL with no signature linked.
    let marked = set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            mass_status: Some(Some(MassStatus::Critical)),
            time_status: Some(Some(TimeStatus::Eol)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(marked.mass_status, Some(MassStatus::Critical));
    assert_eq!(marked.time_status, Some(TimeStatus::Eol));

    // Partial: clear only the time, leave mass untouched.
    let cleared = set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            time_status: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        cleared.mass_status,
        Some(MassStatus::Critical),
        "mass left unchanged"
    );
    assert_eq!(cleared.time_status, None, "time cleared");

    // Unknown connection → NotFound.
    assert!(matches!(
        set_connection_status(
            &pool,
            w.owner,
            SetConnectionStatus {
                map_id: w.map_id,
                connection_id: 4242,
                mass_status: Some(Some(MassStatus::Stable)),
                ..Default::default()
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}
