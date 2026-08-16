//! Signatures: CRUD, linking a wormhole sig to a connection, and the connection<->signature
//! state sync (worst-wins on link, verbatim propagation on edit).

mod common;

use common::{SYS_A, SYS_B, member_with_role, world};
use sqlx::PgPool;
use vector::maps::connection::{
    AddConnection, SetConnectionStatus, add_connection, set_connection_status,
};
use vector::maps::map::{GetMap, get_map};
use vector::maps::signatures::{
    AddSignature, LinkSignature, RemoveSignature, Signature, UnlinkSignature, UpdateSignature,
    add_signature, link_signature, list_signatures, remove_signature, unlink_signature,
    update_signature,
};
use vector::maps::solar_system::{AddSystem, add_system};
use vector::maps::{Actor, MapError, MassStatus, Role, SignatureGroup, TimeStatus};

/// Place `system` on the map; return its `map_solar_systems` id.
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

#[sqlx::test]
async fn add_validates_and_persists(pool: PgPool) {
    let w = world(&pool).await;
    place(&pool, w.owner, w.map_id, SYS_A).await;
    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;

    // A viewer can't scan.
    assert!(matches!(
        add_signature(
            &pool,
            viewer,
            AddSignature {
                map_id: w.map_id,
                solar_system_id: SYS_A,
                signature_id: "ABC-123".into(),
                group: SignatureGroup::Wormhole,
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            mass_status: Some(MassStatus::Critical),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(sig.signature_id, "ABC-123");
    assert_eq!(sig.group, SignatureGroup::Wormhole);
    assert_eq!(sig.mass_status, Some(MassStatus::Critical));
    assert_eq!(sig.connection_id, None);

    // A non-wormhole sig may not carry wormhole state.
    assert!(matches!(
        add_signature(
            &pool,
            w.owner,
            AddSignature {
                map_id: w.map_id,
                solar_system_id: SYS_A,
                signature_id: "DAT-001".into(),
                group: SignatureGroup::Data,
                mass_status: Some(MassStatus::Stable),
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    // Duplicate id in the same system → Conflict.
    assert!(matches!(
        add_signature(
            &pool,
            w.owner,
            AddSignature {
                map_id: w.map_id,
                solar_system_id: SYS_A,
                signature_id: "ABC-123".into(),
                group: SignatureGroup::Wormhole,
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Conflict(_)),
    ));

    // A signature for a system not placed on the map → Validation.
    assert!(matches!(
        add_signature(
            &pool,
            w.owner,
            AddSignature {
                map_id: w.map_id,
                solar_system_id: SYS_B,
                signature_id: "ZZZ-999".into(),
                group: SignatureGroup::Wormhole,
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));
}

#[sqlx::test]
async fn update_and_remove(pool: PgPool) {
    let w = world(&pool).await;
    place(&pool, w.owner, w.map_id, SYS_A).await;
    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let updated = update_signature(
        &pool,
        w.owner,
        UpdateSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
            name: Some(Some("C247".into())),
            time_status: Some(Some(TimeStatus::Eol)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name.as_deref(), Some("C247"));
    assert_eq!(updated.time_status, Some(TimeStatus::Eol));

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

    // Removing an unknown signature → NotFound.
    assert!(matches!(
        remove_signature(
            &pool,
            w.owner,
            RemoveSignature {
                map_id: w.map_id,
                signature_pk: 4242
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}

/// The core sync contract: linking merges to the worst state per field (crossing in *both*
/// directions), then later edits propagate verbatim — including downgrades.
#[sqlx::test]
async fn link_merges_worst_then_edits_propagate(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B).await;
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: vector::maps::ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    // Connection marked: mass critical, time stable.
    set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            mass_status: Some(Some(MassStatus::Critical)),
            time_status: Some(Some(TimeStatus::Stable)),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // A scanned wormhole sig in A: mass stable, time eol — disagrees with the connection on
    // *both* fields, each side worse on a different one.
    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            mass_status: Some(MassStatus::Stable),
            time_status: Some(TimeStatus::Eol),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Link → merge: mass = critical (from the connection), time = eol (from the sig).
    let linked = link_signature(
        &pool,
        w.owner,
        LinkSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
            connection_id: conn.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(linked.connection_id, Some(conn.id));
    assert_eq!(
        linked.mass_status,
        Some(MassStatus::Critical),
        "sig pulled up from connection"
    );
    assert_eq!(linked.time_status, Some(TimeStatus::Eol));
    // And the connection adopted the sig's worse time.
    let c = connection_state(&pool, w.owner, w.map_id, conn.id).await;
    assert_eq!(c.mass_status, Some(MassStatus::Critical));
    assert_eq!(
        c.time_status,
        Some(TimeStatus::Eol),
        "connection pulled up from sig"
    );

    // Downgrade the connection to fully stable → the sig follows (verbatim, not worst-wins).
    set_connection_status(
        &pool,
        w.owner,
        SetConnectionStatus {
            map_id: w.map_id,
            connection_id: conn.id,
            mass_status: Some(Some(MassStatus::Stable)),
            time_status: Some(Some(TimeStatus::Stable)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let s = only_signature(&pool, w.owner, w.map_id).await;
    assert_eq!(
        s.mass_status,
        Some(MassStatus::Stable),
        "downgrade propagated to sig"
    );
    assert_eq!(s.time_status, Some(TimeStatus::Stable));

    // Edit from the signature side → the connection follows.
    update_signature(
        &pool,
        w.owner,
        UpdateSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
            mass_status: Some(Some(MassStatus::Reduced)),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let c = connection_state(&pool, w.owner, w.map_id, conn.id).await;
    assert_eq!(
        c.mass_status,
        Some(MassStatus::Reduced),
        "sig edit propagated to connection"
    );
}

#[sqlx::test]
async fn link_validations_and_unlink(pool: PgPool) {
    let w = world(&pool).await;
    let a = place(&pool, w.owner, w.map_id, SYS_A).await;
    let b = place(&pool, w.owner, w.map_id, SYS_B).await;
    let conn = add_connection(
        &pool,
        w.owner,
        AddConnection {
            map_id: w.map_id,
            from_system: a,
            to_system: b,
            kind: vector::maps::ConnectionType::Wormhole,
            size: None,
        },
    )
    .await
    .unwrap();

    // A non-wormhole sig can't link.
    let data = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "DAT-001".into(),
            group: SignatureGroup::Data,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        link_signature(
            &pool,
            w.owner,
            LinkSignature {
                map_id: w.map_id,
                signature_pk: data.id,
                connection_id: conn.id
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    // A wormhole sig links and then unlinks, keeping its state as a standalone sig.
    let sig = add_signature(
        &pool,
        w.owner,
        AddSignature {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            signature_id: "ABC-123".into(),
            group: SignatureGroup::Wormhole,
            time_status: Some(TimeStatus::Eol),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    link_signature(
        &pool,
        w.owner,
        LinkSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
            connection_id: conn.id,
        },
    )
    .await
    .unwrap();
    let unlinked = unlink_signature(
        &pool,
        w.owner,
        UnlinkSignature {
            map_id: w.map_id,
            signature_pk: sig.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(unlinked.connection_id, None);
    assert_eq!(
        unlinked.time_status,
        Some(TimeStatus::Eol),
        "state survives unlink"
    );
}

/// Read the connection's current state back through `get_map`.
async fn connection_state(
    pool: &PgPool,
    actor: Actor,
    map_id: i64,
    connection_id: i64,
) -> vector::maps::MapConnection {
    get_map(pool, actor, GetMap { map_id })
        .await
        .unwrap()
        .connections
        .into_iter()
        .find(|c| c.id == connection_id)
        .expect("connection present")
}

async fn only_signature(pool: &PgPool, actor: Actor, map_id: i64) -> Signature {
    let mut sigs = list_signatures(pool, actor, map_id).await.unwrap();
    assert_eq!(sigs.len(), 1);
    sigs.pop().unwrap()
}
