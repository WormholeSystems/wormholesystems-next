//! Solar systems: placing / moving / removing / aliasing systems on a map.

mod common;

use common::{SYS_A, member_with_role, world};
use sqlx::PgPool;
use vector::maps::solar_system::{
    AddSystem, MoveSystem, RemoveSystem, SetAlias, add_system, move_system, remove_system,
    set_alias,
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
    assert_eq!(placed.solar_system_id, SYS_A);
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
