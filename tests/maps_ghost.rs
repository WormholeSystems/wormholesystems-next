//! Ghost placements: the far side of a wormhole before anyone has been through it, and
//! what happens when someone finally says what it is.

mod common;

use common::{SYS_A, SYS_B, SYS_C, member_with_role, world};
use sqlx::PgPool;
use vector::maps::events_log::{MapIdBody, undo};
use vector::maps::ghost::{
    AddGhostSystem, ResolveGhostSystem, add_ghost_system, resolve_ghost_system,
};
use vector::maps::map::{GetMap, get_map};
use vector::maps::signatures::{AddSignature, add_signature, list_signatures};
use vector::maps::solar_system::{AddSystem, add_system};
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
