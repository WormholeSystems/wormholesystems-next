//! The map command journal: that every mutation records an entry, and that an entry's
//! inverse actually restores what it described.

mod common;

use common::{SYS_A, SYS_B, SYS_C, member_with_role, world};
use sqlx::PgPool;
use vector::maps::connection::{
    AddConnection, RemoveConnection, add_connection, remove_connection,
};
use vector::maps::connection::{SetConnectionStatus, set_connection_status};
use vector::maps::events_log::{UndoMapEvent, list_events, undo};
use vector::maps::map::{GetMap, get_map};
use vector::maps::signatures::{
    AddSignature, RemoveSignature, add_signature, list_signatures, remove_signature,
};
use vector::maps::solar_system::{
    AddSystem, RemoveSystem, RemoveSystems, SetAlias, add_system, remove_system, remove_systems,
    set_alias,
};
use vector::maps::{
    Actor, ConnectionType, MapConnection, MapError, MapSystemView, MassStatus, Role, SignatureGroup,
};

async fn place(pool: &PgPool, actor: Actor, map_id: i64, sys: i64, x: f64) -> i64 {
    add_system(
        pool,
        actor,
        AddSystem {
            map_id,
            solar_system_id: sys,
            x,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn systems(pool: &PgPool, actor: Actor, map_id: i64) -> Vec<MapSystemView> {
    get_map(pool, actor, GetMap { map_id })
        .await
        .unwrap()
        .systems
}

async fn connections(pool: &PgPool, actor: Actor, map_id: i64) -> Vec<MapConnection> {
    get_map(pool, actor, GetMap { map_id })
        .await
        .unwrap()
        .connections
}

/// The newest journal entry.
async fn head(pool: &PgPool, actor: Actor, map_id: i64) -> vector::maps::events_log::MapEventEntry {
    list_events(pool, actor, map_id).await.unwrap().remove(0)
}

#[sqlx::test]
async fn every_mutation_records_an_entry(pool: PgPool) {
    let w = world(&pool).await;

    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    set_alias(
        &pool,
        w.owner,
        SetAlias {
            map_id: w.map_id,
            map_solar_system_id: a,
            alias: Some("Home".into()),
        },
    )
    .await
    .unwrap();
    add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    let events = list_events(&pool, w.owner, w.map_id).await.unwrap();
    // Newest first. Creating the map is not itself a map command.
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].kind, "connections.added");
    assert_eq!(events[3].kind, "systems.added");
    for e in &events {
        assert!(!e.label.is_empty(), "every entry needs a label");
        assert_eq!(e.character_id, Some(w.owner.character_id));
        assert_eq!(e.character_name.as_deref(), Some("Char 1001"));
        assert!(e.undoable, "a fresh character-made entry is undoable");
        assert!(e.undone_at.is_none());
    }
}

#[sqlx::test]
async fn undo_restores_a_removed_system_with_its_edges_and_signatures(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();
    add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_B,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: b,
        },
    )
    .await
    .unwrap();
    assert_eq!(systems(&pool, w.owner, w.map_id).await.len(), 1);
    assert!(
        connections(&pool, w.owner, w.map_id).await.is_empty(),
        "removing a system takes its edges with it"
    );

    let ev = head(&pool, w.owner, w.map_id).await;
    assert_eq!(ev.kind, "systems.removed");
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();

    let after = systems(&pool, w.owner, w.map_id).await;
    assert_eq!(after.len(), 2);
    let restored = after.iter().find(|s| s.solar_system_id == SYS_B).unwrap();
    // The placement keeps its id, so anything referencing it still resolves.
    assert_eq!(restored.id, b);
    assert_eq!(
        connections(&pool, w.owner, w.map_id).await.len(),
        1,
        "the cascading edge comes back too"
    );
    let sigs = list_signatures(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].signature_id, "ABC-123");
}

#[sqlx::test]
async fn undo_round_trips_a_connection_status_change(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(conn.mass_status, None);

    set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            kind: None,
            mass_status: Some(Some(MassStatus::Critical)),
            time_status: None,
            size: None,
            preserve_mass: None,
        },
    )
    .await
    .unwrap();

    let ev = head(&pool, w.owner, w.map_id).await;
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();

    // The inverse clears the field rather than leaving it, which is the `Option<Option<_>>`
    // distinction the wire format depends on.
    assert_eq!(
        connections(&pool, w.owner, w.map_id).await[0].mass_status,
        None
    );
}

#[sqlx::test]
async fn redo_is_undoing_the_undo(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    set_alias(
        &pool,
        w.owner,
        SetAlias {
            map_id: w.map_id,
            map_solar_system_id: a,
            alias: Some("Staging".into()),
        },
    )
    .await
    .unwrap();

    let aliased = head(&pool, w.owner, w.map_id).await;
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: aliased.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(systems(&pool, w.owner, w.map_id).await[0].alias, None);

    // The undo recorded its own entry, pointing back at the one it reverted.
    let undo_row = head(&pool, w.owner, w.map_id).await;
    assert_eq!(undo_row.reverts_id, Some(aliased.id));
    assert!(undo_row.undoable);

    // Undoing that entry is the redo.
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: undo_row.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        systems(&pool, w.owner, w.map_id).await[0].alias.as_deref(),
        Some("Staging")
    );
}

#[sqlx::test]
async fn an_entry_can_only_be_undone_once(pool: PgPool) {
    let w = world(&pool).await;
    place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let ev = head(&pool, w.owner, w.map_id).await;

    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        undo(
            &pool,
            w.owner,
            UndoMapEvent {
                map_id: w.map_id,
                event_id: ev.id,
            },
        )
        .await,
        Err(MapError::Conflict(_))
    ));

    let entry = list_events(&pool, w.owner, w.map_id)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.id == ev.id)
        .unwrap();
    assert!(entry.undone_at.is_some());
    assert!(!entry.undoable);
}

#[sqlx::test]
async fn you_cannot_undo_someone_elses_change(pool: PgPool) {
    let w = world(&pool).await;
    let mate = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Member).await;
    place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let ev = head(&pool, mate, w.map_id).await;

    assert!(matches!(
        undo(
            &pool,
            mate,
            UndoMapEvent {
                map_id: w.map_id,
                event_id: ev.id,
            },
        )
        .await,
        Err(MapError::Forbidden)
    ));
}

#[sqlx::test]
async fn history_is_viewer_readable_and_undo_is_member_only(pool: PgPool) {
    let w = world(&pool).await;
    let viewer = member_with_role(&pool, w.owner, w.map_id, 1003, 2003, Role::Viewer).await;
    place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;

    let seen = list_events(&pool, viewer, w.map_id).await.unwrap();
    assert_eq!(seen.len(), 1);

    assert!(matches!(
        undo(
            &pool,
            viewer,
            UndoMapEvent {
                map_id: w.map_id,
                event_id: seen[0].id,
            },
        )
        .await,
        Err(MapError::Forbidden)
    ));
}

#[sqlx::test]
async fn a_bulk_removal_undoes_as_one_entry(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    let c = place(&pool, w.owner, w.map_id, SYS_C, 200.0).await;
    add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    remove_systems(
        &pool,
        w.owner,
        RemoveSystems {
            map_id: w.map_id,
            map_solar_system_ids: vec![a, b, c],
        },
    )
    .await
    .unwrap();
    assert!(systems(&pool, w.owner, w.map_id).await.is_empty());

    let ev = head(&pool, w.owner, w.map_id).await;
    assert_eq!(ev.entries_count, 3, "the entry counts what it touched");

    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(systems(&pool, w.owner, w.map_id).await.len(), 3);
    assert_eq!(connections(&pool, w.owner, w.map_id).await.len(), 1);
}

#[sqlx::test]
async fn undoing_a_signature_removal_brings_it_back(pool: PgPool) {
    let w = world(&pool).await;
    place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "XYZ-999".into(),
            group: SignatureGroup::Data,
            name: Some("Ruined Talocan".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    remove_signature(
        &pool,
        w.owner,
        RemoveSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
        },
    )
    .await
    .unwrap();
    assert!(
        list_signatures(&pool, w.owner, w.map_id)
            .await
            .unwrap()
            .is_empty()
    );

    let ev = head(&pool, w.owner, w.map_id).await;
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();

    let sigs = list_signatures(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].signature_id, "XYZ-999");
    assert_eq!(sigs[0].name.as_deref(), Some("Ruined Talocan"));
    assert_eq!(sigs[0].group, SignatureGroup::Data);
}

#[sqlx::test]
async fn removing_a_connection_and_undoing_it_keeps_the_endpoints(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
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

    let ev = head(&pool, w.owner, w.map_id).await;
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();

    let conns = connections(&pool, w.owner, w.map_id).await;
    assert_eq!(conns.len(), 1);
    assert_eq!(conns[0].from_system, a);
    assert_eq!(conns[0].to_system, b);
    assert_eq!(conns[0].kind, ConnectionType::Wormhole);
    assert_eq!(systems(&pool, w.owner, w.map_id).await.len(), 2);
}

/// The stale sweep deletes edges *and* the placements they strand, so its undo has to put
/// both back. This is the case a placement-only inverse would get wrong: the C endpoint
/// stays (it is pinned), but the edge into it must come back too.
#[sqlx::test]
async fn cleaning_stale_connections_undoes_as_one_entry(pool: PgPool) {
    use vector::maps::connection::{CleanStaleConnections, clean_stale_connections};
    use vector::maps::solar_system::{SetPinned, set_pinned};

    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    let c = place(&pool, w.owner, w.map_id, SYS_C, 200.0).await;
    // C is pinned, so it survives the sweep while B (bare and edgeless after it) does not.
    set_pinned(
        &pool,
        w.owner,
        SetPinned {
            map_id: w.map_id,
            map_solar_system_id: c,
            value: true,
        },
    )
    .await
    .unwrap();

    for (from, to) in [(a, b), (a, c)] {
        let conn = add_connection(
            &pool,
            w.owner,
            AddConnection {
                map_id: w.map_id,
                from_system: from,
                to_system: to,
                kind: ConnectionType::Wormhole,
                size: None,
            },
        )
        .await
        .unwrap();
        set_connection_status(
            &pool,
            w.owner,
            SetConnectionStatus {
                map_id: w.map_id,
                connection_id: conn.id,
                kind: None,
                mass_status: None,
                time_status: Some(Some(vector::maps::TimeStatus::Critical)),
                size: None,
                preserve_mass: None,
            },
        )
        .await
        .unwrap();
    }
    // Age both marks past the sweep threshold.
    sqlx::query("update map_connections set time_status_updated_at = now() - interval '2 hours' where map_id = $1")
        .bind(w.map_id)
        .execute(&pool)
        .await
        .unwrap();

    let removed =
        clean_stale_connections(&pool, w.owner, CleanStaleConnections { map_id: w.map_id })
            .await
            .unwrap();
    assert_eq!(removed, 2);
    assert!(connections(&pool, w.owner, w.map_id).await.is_empty());
    // A and B were bare, so they went with the edges; C is pinned and stays.
    let left = systems(&pool, w.owner, w.map_id).await;
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, c);

    let ev = head(&pool, w.owner, w.map_id).await;
    assert_eq!(ev.kind, "connections.cleaned");
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: ev.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(systems(&pool, w.owner, w.map_id).await.len(), 3);
    assert_eq!(
        connections(&pool, w.owner, w.map_id).await.len(),
        2,
        "the edge into the pinned system must come back, not just the orphans' edges"
    );

    // And redoing the sweep clears them again.
    let redo = head(&pool, w.owner, w.map_id).await;
    undo(
        &pool,
        w.owner,
        UndoMapEvent {
            map_id: w.map_id,
            event_id: redo.id,
        },
    )
    .await
    .unwrap();
    assert!(connections(&pool, w.owner, w.map_id).await.is_empty());
    assert_eq!(systems(&pool, w.owner, w.map_id).await.len(), 1);
}

/// A fresh critical mark is not stale: sweeping must not touch a hole someone just flagged.
#[sqlx::test]
async fn a_freshly_critical_connection_is_not_swept(pool: PgPool) {
    use vector::maps::connection::{CleanStaleConnections, clean_stale_connections};

    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B, 100.0).await;
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();
    set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            kind: None,
            mass_status: None,
            time_status: Some(Some(vector::maps::TimeStatus::Critical)),
            size: None,
            preserve_mass: None,
        },
    )
    .await
    .unwrap();

    let err =
        clean_stale_connections(&pool, w.owner, CleanStaleConnections { map_id: w.map_id }).await;
    assert!(matches!(err, Err(MapError::Conflict(_))));
    assert_eq!(connections(&pool, w.owner, w.map_id).await.len(), 1);
}
