//! Solar systems: placing / moving / removing / aliasing systems on a map.

mod common;

use common::{SYS_A, SYS_B, SYS_C, member_with_role, world};
use sqlx::PgPool;
use vector::maps::solar_system::{
    AddSystem, MoveSystem, MoveSystems, RemoveSystem, RemoveSystems, SetAlias, SetHome, SetPinned,
    SystemMove, add_system, move_system, move_systems, remove_system, remove_systems, set_alias,
    set_home, set_pinned,
};
use vector::maps::{MapError, Role};

#[sqlx::test]
async fn add_system_returns_fields_and_persists(pool: PgPool) {
    let w = world(&pool).await;
    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;

    // A viewer can't edit the graph.
    assert!(matches!(
        add_system(
            &pool,
            viewer,
            AddSystem {
                map_id: w.map_id,
                solar_system_id: SYS_A,
                x: 0.0,
                y: 0.0,
                alias: None
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    let placed = add_system(
        &pool,
        w.owner,
        AddSystem {
            map_id: w.map_id,
            solar_system_id: SYS_A,
            x: 1.5,
            y: -2.5,
            alias: Some("Staging".into()),
        },
    )
    .await
    .unwrap();
    // Return value reflects the inputs exactly.
    assert_eq!(placed.map_id, w.map_id);
    assert_eq!(placed.solar_system_id, Some(SYS_A));
    assert_eq!((placed.position_x, placed.position_y), (1.5, -2.5));
    assert_eq!(placed.alias.as_deref(), Some("Staging"));
    // And the row matches.
    let (sid, x, y, alias): (i64, f64, f64, Option<String>) = sqlx::query_as(
        "select solar_system_id, position_x, position_y, alias from map_solar_systems where id = $1",
    )
    .bind(placed.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (sid, x, y, alias.as_deref()),
        (SYS_A, 1.5, -2.5, Some("Staging"))
    );

    // Duplicate placement → Conflict.
    assert!(matches!(
        add_system(
            &pool,
            w.owner,
            AddSystem {
                map_id: w.map_id,
                solar_system_id: SYS_A,
                x: 5.0,
                y: 5.0,
                alias: None
            }
        )
        .await,
        Err(MapError::Conflict(_)),
    ));
    // Unknown system id → Validation.
    assert!(matches!(
        add_system(
            &pool,
            w.owner,
            AddSystem {
                map_id: w.map_id,
                solar_system_id: 88888888,
                x: 0.0,
                y: 0.0,
                alias: None
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));
}

#[sqlx::test]
async fn remove_system_keeps_details(pool: PgPool) {
    let w = world(&pool).await;
    let placed = add_system(
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
    sqlx::query(
        "insert into map_solar_system_details (map_id, solar_system_id, status) values ($1, $2, 'hostile')",
    )
    .bind(w.map_id)
    .bind(SYS_A)
    .execute(&pool)
    .await
    .unwrap();

    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: placed.id,
        },
    )
    .await
    .unwrap();

    let placements: i64 =
        sqlx::query_scalar("select count(*) from map_solar_systems where map_id = $1")
            .bind(w.map_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(placements, 0);
    let details: i64 = sqlx::query_scalar(
        "select count(*) from map_solar_system_details where map_id = $1 and solar_system_id = $2",
    )
    .bind(w.map_id)
    .bind(SYS_A)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(details, 1, "details should survive system removal");

    // A placement not on this map → NotFound.
    assert!(matches!(
        remove_system(
            &pool,
            w.owner,
            RemoveSystem {
                map_id: w.map_id,
                map_solar_system_id: 4242
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}

#[sqlx::test]
async fn move_and_alias_update_the_row(pool: PgPool) {
    let w = world(&pool).await;
    let placed = add_system(
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

    move_system(
        &pool,
        w.owner,
        MoveSystem {
            map_id: w.map_id,
            map_solar_system_id: placed.id,
            x: 3.0,
            y: 4.0,
        },
    )
    .await
    .unwrap();
    set_alias(
        &pool,
        w.owner,
        SetAlias {
            map_id: w.map_id,
            map_solar_system_id: placed.id,
            alias: Some("Home".into()),
        },
    )
    .await
    .unwrap();

    let (x, y, alias): (f64, f64, Option<String>) =
        sqlx::query_as("select position_x, position_y, alias from map_solar_systems where id = $1")
            .bind(placed.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((x, y), (3.0, 4.0));
    assert_eq!(alias.as_deref(), Some("Home"));

    // Clearing the alias.
    set_alias(
        &pool,
        w.owner,
        SetAlias {
            map_id: w.map_id,
            map_solar_system_id: placed.id,
            alias: None,
        },
    )
    .await
    .unwrap();
    let alias: Option<String> =
        sqlx::query_scalar("select alias from map_solar_systems where id = $1")
            .bind(placed.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(alias, None);

    // Unknown placement → NotFound.
    assert!(matches!(
        move_system(
            &pool,
            w.owner,
            MoveSystem {
                map_id: w.map_id,
                map_solar_system_id: 4242,
                x: 1.0,
                y: 1.0
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}

/// What "pinned" and "home" protect, exactly.
///
/// Pinned means the system does not move and does not get deleted. Home means it does not
/// get deleted, but it can still be dragged around. Neither is an error to try: a marquee
/// across the chain is the same gesture as selecting one system, and refusing the whole
/// delete because the box happened to contain the home system would punish the gesture.
#[sqlx::test]
async fn pinned_and_home_systems_are_passed_over_rather_than_refused(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let pinned = place(&pool, w.owner, w.map_id, SYS_B, 200.0).await;
    let ordinary = place(&pool, w.owner, w.map_id, SYS_C, 400.0).await;
    mark_home(&pool, w.owner, w.map_id, home).await;
    pin(&pool, w.owner, w.map_id, pinned, true).await;

    // A selection spanning all three takes only the one that is not protected, and says so
    // by succeeding rather than by erroring.
    let removed = remove_systems(
        &pool,
        w.owner,
        RemoveSystems {
            map_id: w.map_id,
            map_solar_system_ids: vec![home, pinned, ordinary],
        },
    )
    .await
    .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(placed_ids(&pool, w.owner, w.map_id).await.len(), 2);

    // Selecting only protected systems is a quiet no-op, not a failure.
    let removed = remove_systems(
        &pool,
        w.owner,
        RemoveSystems {
            map_id: w.map_id,
            map_solar_system_ids: vec![home, pinned],
        },
    )
    .await
    .unwrap();
    assert_eq!(removed, 0);
    assert_eq!(placed_ids(&pool, w.owner, w.map_id).await.len(), 2);

    // Asking for one of them on its own is the same: nothing happens, nothing complains.
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
    assert_eq!(placed_ids(&pool, w.owner, w.map_id).await.len(), 2);
}

#[sqlx::test]
async fn unpinning_gives_the_system_back(pool: PgPool) {
    let w = world(&pool).await;
    let pinned = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    pin(&pool, w.owner, w.map_id, pinned, true).await;

    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: pinned,
        },
    )
    .await
    .unwrap();
    assert_eq!(placed_ids(&pool, w.owner, w.map_id).await.len(), 1);

    // A guard, not a life sentence.
    pin(&pool, w.owner, w.map_id, pinned, false).await;
    remove_system(
        &pool,
        w.owner,
        RemoveSystem {
            map_id: w.map_id,
            map_solar_system_id: pinned,
        },
    )
    .await
    .unwrap();
    assert!(placed_ids(&pool, w.owner, w.map_id).await.is_empty());
}

#[sqlx::test]
async fn a_pinned_system_will_not_move_but_the_home_system_will(pool: PgPool) {
    let w = world(&pool).await;
    let home = place(&pool, w.owner, w.map_id, SYS_A, 0.0).await;
    let pinned = place(&pool, w.owner, w.map_id, SYS_B, 200.0).await;
    mark_home(&pool, w.owner, w.map_id, home).await;
    pin(&pool, w.owner, w.map_id, pinned, true).await;

    // Being home says nothing about moving.
    move_system(
        &pool,
        w.owner,
        MoveSystem {
            map_id: w.map_id,
            map_solar_system_id: home,
            x: 640.0,
            y: 80.0,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        position(&pool, w.owner, w.map_id, home).await,
        (640.0, 80.0)
    );

    // Pinning does, and the server holds it rather than trusting the client's drag lock.
    move_system(
        &pool,
        w.owner,
        MoveSystem {
            map_id: w.map_id,
            map_solar_system_id: pinned,
            x: 900.0,
            y: 900.0,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        position(&pool, w.owner, w.map_id, pinned).await,
        (200.0, 0.0)
    );

    // Dragging a multi-selection moves everything in it except the pinned one.
    move_systems(
        &pool,
        w.owner,
        MoveSystems {
            map_id: w.map_id,
            moves: vec![
                SystemMove {
                    map_solar_system_id: home,
                    x: 20.0,
                    y: 20.0,
                },
                SystemMove {
                    map_solar_system_id: pinned,
                    x: 20.0,
                    y: 20.0,
                },
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(position(&pool, w.owner, w.map_id, home).await, (20.0, 20.0));
    assert_eq!(
        position(&pool, w.owner, w.map_id, pinned).await,
        (200.0, 0.0)
    );
}

async fn mark_home(pool: &PgPool, actor: vector::maps::Actor, map_id: i64, id: i64) {
    set_home(
        pool,
        actor,
        SetHome {
            map_id,
            map_solar_system_id: id,
            value: true,
        },
    )
    .await
    .unwrap();
}

async fn pin(pool: &PgPool, actor: vector::maps::Actor, map_id: i64, id: i64, value: bool) {
    set_pinned(
        pool,
        actor,
        SetPinned {
            map_id,
            map_solar_system_id: id,
            value,
        },
    )
    .await
    .unwrap();
}

async fn position(pool: &PgPool, actor: vector::maps::Actor, map_id: i64, id: i64) -> (f64, f64) {
    let view = vector::maps::map::get_map(pool, actor, vector::maps::map::GetMap { map_id })
        .await
        .unwrap();
    let s = view.systems.iter().find(|s| s.id == id).expect("placed");
    (s.position_x, s.position_y)
}

async fn place(pool: &PgPool, actor: vector::maps::Actor, map_id: i64, sys: i64, x: f64) -> i64 {
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

async fn placed_ids(pool: &PgPool, actor: vector::maps::Actor, map_id: i64) -> Vec<i64> {
    vector::maps::map::get_map(pool, actor, vector::maps::map::GetMap { map_id })
        .await
        .unwrap()
        .systems
        .iter()
        .map(|s| s.id)
        .collect()
}
