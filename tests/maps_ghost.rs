//! Ghost placements: the far side of a wormhole before anyone has been through it, and
//! what happens when someone finally says what it is.

mod common;

use common::{SYS_A, SYS_B, SYS_C, member_with_role, world};
use sqlx::PgPool;
use vector::maps::connection::{RemoveConnection, remove_connection};
use vector::maps::events_log::{MapIdBody, undo};
use vector::maps::ghost::{
    AddGhostSystem, ResolveGhostSystem, add_ghost_system, resolve_ghost_system,
};
use vector::maps::map::{GetMap, get_map};
use vector::maps::signatures::{
    AddSignature, PasteSignatures, PastedSignature, UpdateSignature, add_signature,
    list_signatures, paste_signatures, update_signature,
};
use vector::maps::solar_system::{
    AddSystem, RemoveSystem, SetPinned, add_system, remove_system, set_pinned,
};
use vector::maps::{Actor, MapError, Role, SignatureGroup};

async fn place(pool: &PgPool, actor: Actor, map_id: i64, system: i64) -> i64 {
    add_system(
        pool,
        actor,
        AddSystem {
            map_id,
            solar_system_id: system,
            x: 0.0,
            y: 0.0,
            alias: None,
        },
    )
    .await
    .unwrap()
    .id
}

async fn scan(pool: &PgPool, actor: Actor, map_id: i64, system: i64, sig: &str) -> i64 {
    add_signature(
        pool,
        actor,
        AddSignature {
            map_id,
            solar_system_id: system,
            signature_id: sig.into(),
            group: SignatureGroup::Wormhole,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
}

#[sqlx::test]
async fn a_ghost_is_a_placement_with_no_system_hanging_off_the_scan(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let sig = scan(&pool, w.owner, w.map_id, SYS_A, "ABC-123").await;

    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;
    assert!(matches!(
        add_ghost_system(
            &pool,
            viewer,
            AddGhostSystem {
                map_id: w.map_id,
                from_system: home,
                signature_pk: Some(sig),
                x: 10.0,
                y: 20.0,
                alias: None,
                size: None,
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: Some(sig),
            x: 10.0,
            y: 20.0,
            alias: Some("1a".into()),
            size: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(ghost.solar_system_id, None);
    assert_eq!(ghost.alias.as_deref(), Some("1a"));

    // It is on the map like any other node, with an edge from where it was scanned.
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    let placed = view.systems.iter().find(|s| s.id == ghost.id).unwrap();
    assert_eq!(placed.name, None, "nothing to name it after yet");
    assert_eq!(placed.region, None);
    assert!(placed.statics.is_empty());
    assert_eq!(view.connections.len(), 1);
    assert_eq!(view.connections[0].from_system, home);
    assert_eq!(view.connections[0].to_system, ghost.id);

    // And the signature it came from is linked to that edge, so the two stay in step.
    let sigs = list_signatures(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(sigs[0].connection_id, Some(view.connections[0].id));

    // A second ghost for the same signature would be that hole twice over.
    assert!(matches!(
        add_ghost_system(
            &pool,
            w.owner,
            AddGhostSystem {
                map_id: w.map_id,
                from_system: home,
                signature_pk: Some(sig),
                x: 30.0,
                y: 40.0,
                alias: None,
                size: None,
            }
        )
        .await,
        Err(MapError::Conflict(_)),
    ));
}

#[sqlx::test]
async fn resolving_names_the_ghost_and_keeps_where_it_sits(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: Some("1a".into()),
            size: None,
        },
    )
    .await
    .unwrap();

    let resolved = resolve_ghost_system(
        &pool,
        w.owner,
        ResolveGhostSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            solar_system_id: Some(SYS_B),
        },
    )
    .await
    .unwrap();

    assert_eq!(resolved.id, ghost.id, "the node keeps its identity");
    assert_eq!(resolved.solar_system_id, Some(SYS_B));
    assert_eq!((resolved.position_x, resolved.position_y), (10.0, 20.0));
    assert_eq!(resolved.alias.as_deref(), Some("1a"));

    // Only ghosts are resolvable; a real system is not re-pointed at another one.
    assert!(matches!(
        resolve_ghost_system(
            &pool,
            w.owner,
            ResolveGhostSystem {
                map_id: w.map_id,
                map_solar_system_id: ghost.id,
                solar_system_id: Some(SYS_C),
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));
}

#[sqlx::test]
async fn a_ghost_that_leads_back_into_the_chain_is_merged(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let far = place(&pool, w.owner, w.map_id, SYS_C).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();

    // The hole turns out to lead to a system already on the map, from the other side.
    let resolved = resolve_ghost_system(
        &pool,
        w.owner,
        ResolveGhostSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            solar_system_id: Some(SYS_C),
        },
    )
    .await
    .unwrap();

    assert_eq!(resolved.id, far, "merged into the placement already there");

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 2, "the ghost is gone, not duplicated");
    assert!(view.systems.iter().all(|s| s.id != ghost.id));
    assert_eq!(view.connections.len(), 1);
    assert_eq!(
        (
            view.connections[0].from_system,
            view.connections[0].to_system
        ),
        (home, far),
        "the edge moved onto the real system"
    );
}

#[sqlx::test]
async fn a_ghost_cannot_lead_to_the_system_it_hangs_off(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();

    assert!(matches!(
        resolve_ghost_system(
            &pool,
            w.owner,
            ResolveGhostSystem {
                map_id: w.map_id,
                map_solar_system_id: ghost.id,
                solar_system_id: Some(SYS_A),
            }
        )
        .await,
        Err(MapError::Conflict(_)),
    ));
}

#[sqlx::test]
async fn undoing_a_merge_puts_the_ghost_back_with_its_edge(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    place(&pool, w.owner, w.map_id, SYS_C).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: Some("1a".into()),
            size: None,
        },
    )
    .await
    .unwrap();
    resolve_ghost_system(
        &pool,
        w.owner,
        ResolveGhostSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            solar_system_id: Some(SYS_C),
        },
    )
    .await
    .unwrap();

    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    let back = view.systems.iter().find(|s| s.id == ghost.id).unwrap();
    assert_eq!(back.solar_system_id, None);
    assert_eq!(back.alias.as_deref(), Some("1a"));
    assert_eq!(view.connections.len(), 1);
    assert_eq!(
        (
            view.connections[0].from_system,
            view.connections[0].to_system
        ),
        (home, ghost.id),
        "the edge went back to the ghost"
    );
}

#[sqlx::test]
async fn undoing_a_resolve_makes_it_a_ghost_again(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();
    resolve_ghost_system(
        &pool,
        w.owner,
        ResolveGhostSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            solar_system_id: Some(SYS_B),
        },
    )
    .await
    .unwrap();

    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    let back = view.systems.iter().find(|s| s.id == ghost.id).unwrap();
    assert_eq!(back.solar_system_id, None);
}

#[sqlx::test]
async fn a_pasted_scan_raises_the_holes_it_found(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    // Off by default: a scan changes nothing but the signature list.
    paste_signatures(
        &pool,
        w.owner,
        PasteSignatures {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signatures: vec![PastedSignature {
                signature_id: "ABC-123".into(),
                group: Some(SignatureGroup::Wormhole),
                signature_type_id: None,
                name: None,
            }],
        },
    )
    .await
    .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1);

    sqlx::query("update maps set ghost_unlinked_wormholes = true where id = $1")
        .bind(w.map_id)
        .execute(&pool)
        .await
        .unwrap();

    // Two more holes, and the one already scanned: every unmapped hole gets a node, and
    // the whole thing is one entry in the history.
    paste_signatures(
        &pool,
        w.owner,
        PasteSignatures {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signatures: vec![
                PastedSignature {
                    signature_id: "DEF-456".into(),
                    group: Some(SignatureGroup::Wormhole),
                    signature_type_id: None,
                    name: None,
                },
                PastedSignature {
                    signature_id: "GHI-789".into(),
                    group: Some(SignatureGroup::Wormhole),
                    signature_type_id: None,
                    name: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    let ghosts: Vec<_> = view
        .systems
        .iter()
        .filter(|s| s.solar_system_id.is_none())
        .collect();
    assert_eq!(
        ghosts.len(),
        3,
        "one per unmapped hole, the older one included"
    );
    assert_eq!(view.connections.len(), 3);
    assert!(view.connections.iter().all(|c| c.from_system == home));
    // Siblings stack in the column beside the system they hang off.
    assert!(ghosts.iter().all(|g| g.position_x == ghosts[0].position_x));
    let mut ys: Vec<i64> = ghosts.iter().map(|g| g.position_y as i64).collect();
    ys.sort_unstable();
    ys.dedup();
    assert_eq!(ys.len(), 3, "and do not sit on top of each other");

    // Every signature is linked to the node it raised.
    let sigs = list_signatures(&pool, w.owner, w.map_id).await.unwrap();
    assert!(sigs.iter().all(|s| s.connection_id.is_some()));

    // One undo takes the scan and its nodes back together.
    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1, "the paste's nodes went with it");
    assert!(view.connections.is_empty());
}

#[sqlx::test]
async fn calling_a_signature_a_wormhole_raises_one_too(pool: PgPool) {
    let w = world(&pool).await;
    place(&pool, w.owner, w.map_id, SYS_A).await;
    sqlx::query("update maps set ghost_unlinked_wormholes = true where id = $1")
        .bind(w.map_id)
        .execute(&pool)
        .await
        .unwrap();

    // Typed in by hand and uncategorised: nothing is known about it, so nothing is drawn.
    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Unknown,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1);

    update_signature(
        &pool,
        w.owner,
        UpdateSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
            group: Some(SignatureGroup::Wormhole),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 2);
    assert!(view.systems.iter().any(|s| s.solar_system_id.is_none()));
}

#[sqlx::test]
async fn removing_a_system_takes_the_holes_hanging_off_it(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let far = place(&pool, w.owner, w.map_id, SYS_C).await;
    for sig in ["ABC-123", "DEF-456"] {
        let pk = scan(&pool, w.owner, w.map_id, SYS_A, sig).await;
        add_ghost_system(
            &pool,
            w.owner,
            AddGhostSystem {
                map_id: w.map_id,
                from_system: home,
                signature_pk: Some(pk),
                x: 10.0,
                y: 20.0,
                alias: None,
                size: None,
            },
        )
        .await
        .unwrap();
    }

    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: home,
        },
    )
    .await
    .unwrap();

    // The scanned system is gone, and so are the holes that only existed as its far
    // sides. The unrelated system stays.
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1);
    assert_eq!(view.systems[0].id, far);
    assert!(view.connections.is_empty());

    // And one undo puts the system and its holes back together.
    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 4);
    assert_eq!(
        view.systems
            .iter()
            .filter(|s| s.solar_system_id.is_none())
            .count(),
        2
    );
    assert_eq!(view.connections.len(), 2);
}

#[sqlx::test]
async fn a_ghost_goes_with_the_connection_that_made_it(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    let connection = view.connections[0].id;

    remove_connection(
        &pool,
        w.owner,
        RemoveConnection {
            map_id: w.map_id,
            connection_id: connection,
        },
    )
    .await
    .unwrap();

    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1, "the hole had nothing else to be");
    assert!(view.systems.iter().all(|s| s.id != ghost.id));

    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 2);
    assert_eq!(view.connections.len(), 1);
}

#[sqlx::test]
async fn a_real_system_left_without_connections_stays(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let far = place(&pool, w.owner, w.map_id, SYS_B).await;
    vector::maps::connection::add_connection(
        &pool,
        w.owner,
        vector::maps::connection::AddConnection {
            map_id: w.map_id,
            from_system: home,
            to_system: far,
            kind: vector::maps::ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: home,
        },
    )
    .await
    .unwrap();

    // Somebody put that system on the map on purpose; losing its last hole is not a
    // reason to take it away.
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.systems.len(), 1);
    assert_eq!(view.systems[0].id, far);
}

#[sqlx::test]
async fn a_hole_nobody_has_been_through_cannot_be_pinned(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: None,
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();

    // Pinning holds a node still, roots the tree layout and is passed over by every
    // sweep, which would leave this one behind when its connection goes.
    assert!(matches!(
        set_pinned(
            &pool,
            w.owner,
            SetPinned {
                map_id: w.map_id,
                map_solar_system_id: ghost.id,
                value: true,
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    // Once it is a system, it pins like any other.
    resolve_ghost_system(
        &pool,
        w.owner,
        ResolveGhostSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            solar_system_id: Some(SYS_B),
        },
    )
    .await
    .unwrap();
    set_pinned(
        &pool,
        w.owner,
        SetPinned {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
            value: true,
        },
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn removing_a_system_takes_the_signature_that_led_to_it(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A).await;
    let sig = scan(&pool, w.owner, w.map_id, SYS_A, "ABC-123").await;
    let ghost = add_ghost_system(
        &pool,
        w.owner,
        AddGhostSystem {
            map_id: w.map_id,
            from_system: home,
            signature_pk: Some(sig),
            x: 10.0,
            y: 20.0,
            alias: None,
            size: None,
        },
    )
    .await
    .unwrap();

    // Taking the far side off the map takes the hole with it: the signature it was
    // scanned as is the same fact, and leaving it behind puts the node back on the next
    // paste.
    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: ghost.id,
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

    // And one undo brings the node, its edge and the signature back, still linked.
    undo(&pool, w.owner, MapIdBody { map_id: w.map_id })
        .await
        .unwrap();
    let sigs = list_signatures(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(sigs.len(), 1);
    let view = get_map(&pool, w.owner, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.connections.len(), 1);
    assert_eq!(sigs[0].connection_id, Some(view.connections[0].id));
}
