//! Behaviour tests for the map action layer, driven against a real Postgres via
//! `#[sqlx::test]` (isolated database per test, migrations applied). Each test seeds the
//! minimal fixtures it needs and asserts the contract in `docs/features/maps.md`.

use sqlx::PgPool;
use vector::maps::access::{effective_role, revoke_access, set_access};
use vector::maps::graph::{
    add_connection, add_system, move_system, remove_connection, remove_system, set_alias,
};
use vector::maps::lifecycle::{MapUpdate, create_map, delete_map, get_map, list_maps, update_map};
use vector::maps::{Actor, ConnectionType, MapError, Role, SubjectType};

// ---- fixtures ----

/// A region → constellation → three solar systems, so systems can be placed.
const SYS_A: i64 = 30000142;
const SYS_B: i64 = 30000144;
const SYS_C: i64 = 30000145;

async fn seed_universe(pool: &PgPool) {
    sqlx::query("insert into regions (id, name) values (10000001, 'Test Region')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("insert into constellations (id, region_id, name) values (20000001, 10000001, 'Test Const')")
        .execute(pool)
        .await
        .unwrap();
    for (id, name) in [
        (SYS_A, "Jita"),
        (SYS_B, "Perimeter"),
        (SYS_C, "New Caldari"),
    ] {
        sqlx::query(
            "insert into solar_systems (id, constellation_id, region_id, name, security_status)
             values ($1, 20000001, 10000001, $2, 0.9)",
        )
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn new_user(pool: &PgPool) -> i64 {
    sqlx::query_scalar("insert into users default values returning id")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn add_character(pool: &PgPool, user_id: i64, id: i64, corp: i64, alliance: Option<i64>) {
    // Satisfy the characters → corporations / alliances FKs.
    sqlx::query("insert into corporations (id, name, ticker) values ($1, $2, 'CORP') on conflict (id) do nothing")
        .bind(corp)
        .bind(format!("Corp {corp}"))
        .execute(pool)
        .await
        .unwrap();
    if let Some(a) = alliance {
        sqlx::query("insert into alliances (id, name, ticker) values ($1, $2, 'ALLY') on conflict (id) do nothing")
            .bind(a)
            .bind(format!("Alliance {a}"))
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "insert into characters (id, user_id, name, owner_hash, corporation_id, alliance_id)
         values ($1, $2, $3, 'hash', $4, $5)",
    )
    .bind(id)
    .bind(user_id)
    .bind(format!("Char {id}"))
    .bind(corp)
    .bind(alliance)
    .execute(pool)
    .await
    .unwrap();
}

/// An owner with a map, on a seeded universe.
struct World {
    owner: Actor,
    map_id: i64,
}

async fn world(pool: &PgPool) -> World {
    seed_universe(pool).await;
    let user = new_user(pool).await;
    add_character(pool, user, 1001, 2001, None).await;
    let owner = Actor {
        user_id: user,
        character_id: 1001,
    };
    let map = create_map(pool, owner, "Chain", None).await.unwrap();
    World {
        owner,
        map_id: map.id,
    }
}

/// Add a fresh user + character and grant them `role` on the map; return their actor.
async fn member_with_role(
    pool: &PgPool,
    granter: Actor,
    map_id: i64,
    char_id: i64,
    corp: i64,
    role: Role,
) -> Actor {
    let user = new_user(pool).await;
    add_character(pool, user, char_id, corp, None).await;
    set_access(pool, granter, map_id, SubjectType::Character, char_id, role)
        .await
        .unwrap();
    Actor {
        user_id: user,
        character_id: char_id,
    }
}

// ---- map lifecycle ----

#[sqlx::test]
async fn create_grants_owner_to_creator(pool: PgPool) {
    let w = world(&pool).await;
    assert_eq!(
        effective_role(&pool, w.map_id, w.owner.user_id)
            .await
            .unwrap(),
        Some(Role::Owner),
    );
    let access_rows: i64 = sqlx::query_scalar("select count(*) from map_access where map_id = $1")
        .bind(w.map_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(access_rows, 1);
}

#[sqlx::test]
async fn create_rejects_blank_name_and_foreign_character(pool: PgPool) {
    seed_universe(&pool).await;
    let user = new_user(&pool).await;
    add_character(&pool, user, 1001, 2001, None).await;
    let actor = Actor {
        user_id: user,
        character_id: 1001,
    };

    assert!(matches!(
        create_map(&pool, actor, "   ", None).await,
        Err(MapError::Validation(_)),
    ));
    // Acting as a character that isn't this user's.
    let stranger = Actor {
        user_id: user,
        character_id: 9999,
    };
    assert!(matches!(
        create_map(&pool, stranger, "Chain", None).await,
        Err(MapError::Forbidden),
    ));
}

#[sqlx::test]
async fn update_is_owner_only_and_patches_fields(pool: PgPool) {
    let w = world(&pool).await;
    let member = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Member).await;

    // Member cannot modify the map itself.
    assert!(matches!(
        update_map(
            &pool,
            member,
            w.map_id,
            MapUpdate {
                name: Some("X".into()),
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    // Owner renames and sets a description.
    let updated = update_map(
        &pool,
        w.owner,
        w.map_id,
        MapUpdate {
            name: Some("Renamed".into()),
            description: Some(Some("home".into())),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.description.as_deref(), Some("home"));

    // Omitted field is unchanged; Some(None) clears.
    let cleared = update_map(
        &pool,
        w.owner,
        w.map_id,
        MapUpdate {
            description: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(cleared.name, "Renamed");
    assert_eq!(cleared.description, None);

    // Blank name rejected.
    assert!(matches!(
        update_map(
            &pool,
            w.owner,
            w.map_id,
            MapUpdate {
                name: Some(" ".into()),
                ..Default::default()
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));
}

#[sqlx::test]
async fn delete_is_owner_only_and_cascades(pool: PgPool) {
    let w = world(&pool).await;
    let member = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Member).await;
    let a = add_system(&pool, member, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();
    let b = add_system(&pool, member, w.map_id, SYS_B, 1.0, 1.0, None)
        .await
        .unwrap();
    add_connection(
        &pool,
        member,
        w.map_id,
        a.id,
        b.id,
        ConnectionType::Wormhole,
    )
    .await
    .unwrap();

    assert!(matches!(
        delete_map(&pool, member, w.map_id).await,
        Err(MapError::Forbidden)
    ));

    delete_map(&pool, w.owner, w.map_id).await.unwrap();
    for table in ["map_solar_systems", "map_connections", "map_access"] {
        let n: i64 = sqlx::query_scalar(&format!("select count(*) from {table} where map_id = $1"))
            .bind(w.map_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} should be empty after delete");
    }
}

#[sqlx::test]
async fn list_returns_accessible_maps_with_highest_role(pool: PgPool) {
    let w = world(&pool).await;

    // A second user, member via their corporation (not their character id).
    let user = new_user(&pool).await;
    add_character(&pool, user, 1002, 2002, None).await;
    set_access(
        &pool,
        w.owner,
        w.map_id,
        SubjectType::Corporation,
        2002,
        Role::Member,
    )
    .await
    .unwrap();
    // Also grant their character viewer — the higher (member, via corp) should win.
    set_access(
        &pool,
        w.owner,
        w.map_id,
        SubjectType::Character,
        1002,
        Role::Viewer,
    )
    .await
    .unwrap();

    let maps = list_maps(&pool, user).await.unwrap();
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].1, Role::Member);

    // A user with no grants sees nothing.
    let outsider = new_user(&pool).await;
    add_character(&pool, outsider, 1003, 2003, None).await;
    assert!(list_maps(&pool, outsider).await.unwrap().is_empty());
}

#[sqlx::test]
async fn get_map_visibility(pool: PgPool) {
    let w = world(&pool).await;
    add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();

    let view = get_map(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(view.map.id, w.map_id);
    assert_eq!(view.systems.len(), 1);

    // No access at all → NotFound (not Forbidden), so existence isn't leaked.
    let outsider = new_user(&pool).await;
    add_character(&pool, outsider, 1003, 2003, None).await;
    let stranger = Actor {
        user_id: outsider,
        character_id: 1003,
    };
    assert!(matches!(
        get_map(&pool, stranger, w.map_id).await,
        Err(MapError::NotFound)
    ));
}

// ---- access management ----

#[sqlx::test]
async fn grant_respects_privilege_ceiling_and_owner_invariant(pool: PgPool) {
    let w = world(&pool).await;
    let manager = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Manager).await;

    // A manager cannot grant owner (above their own role).
    assert!(matches!(
        set_access(
            &pool,
            manager,
            w.map_id,
            SubjectType::Character,
            1003,
            Role::Owner
        )
        .await,
        Err(MapError::Forbidden),
    ));
    // But can grant up to manager.
    set_access(
        &pool,
        manager,
        w.map_id,
        SubjectType::Character,
        1003,
        Role::Member,
    )
    .await
    .unwrap();

    // The sole owner cannot be downgraded — that would orphan the map.
    assert!(matches!(
        set_access(
            &pool,
            w.owner,
            w.map_id,
            SubjectType::Character,
            w.owner.character_id,
            Role::Manager
        )
        .await,
        Err(MapError::LastOwner),
    ));
    // Nor revoked.
    assert!(matches!(
        revoke_access(&pool, w.owner, w.map_id, w.owner.character_id).await,
        Err(MapError::LastOwner),
    ));
}

#[sqlx::test]
async fn revoke_removes_grant(pool: PgPool) {
    let w = world(&pool).await;
    let member = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Member).await;
    assert_eq!(
        effective_role(&pool, w.map_id, member.user_id)
            .await
            .unwrap(),
        Some(Role::Member)
    );

    revoke_access(&pool, w.owner, w.map_id, member.character_id)
        .await
        .unwrap();
    assert_eq!(
        effective_role(&pool, w.map_id, member.user_id)
            .await
            .unwrap(),
        None
    );

    // Revoking a subject that was never granted → NotFound.
    assert!(matches!(
        revoke_access(&pool, w.owner, w.map_id, 4242).await,
        Err(MapError::NotFound),
    ));
}

// ---- graph editing ----

#[sqlx::test]
async fn add_system_validates_and_dedupes(pool: PgPool) {
    let w = world(&pool).await;
    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;

    // Viewer can't edit the graph.
    assert!(matches!(
        add_system(&pool, viewer, w.map_id, SYS_A, 0.0, 0.0, None).await,
        Err(MapError::Forbidden),
    ));

    add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();
    // Duplicate placement.
    assert!(matches!(
        add_system(&pool, w.owner, w.map_id, SYS_A, 5.0, 5.0, None).await,
        Err(MapError::Conflict(_)),
    ));
    // Unknown system id.
    assert!(matches!(
        add_system(&pool, w.owner, w.map_id, 88888888, 0.0, 0.0, None).await,
        Err(MapError::Validation(_)),
    ));
}

#[sqlx::test]
async fn remove_system_keeps_details(pool: PgPool) {
    let w = world(&pool).await;
    let placed = add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();
    // Persisted intel for the system.
    sqlx::query(
        "insert into map_solar_system_details (map_id, solar_system_id, status) values ($1, $2, 'hostile')",
    )
    .bind(w.map_id)
    .bind(SYS_A)
    .execute(&pool)
    .await
    .unwrap();

    remove_system(&pool, w.owner, w.map_id, placed.id)
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

    // Removing a placement that isn't on the map → NotFound.
    assert!(matches!(
        remove_system(&pool, w.owner, w.map_id, 4242).await,
        Err(MapError::NotFound),
    ));
}

#[sqlx::test]
async fn move_and_alias(pool: PgPool) {
    let w = world(&pool).await;
    let placed = add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();

    move_system(&pool, w.owner, w.map_id, placed.id, 3.0, 4.0)
        .await
        .unwrap();
    set_alias(&pool, w.owner, w.map_id, placed.id, Some("Home"))
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

    // Wrong map / unknown placement → NotFound.
    assert!(matches!(
        move_system(&pool, w.owner, w.map_id, 4242, 1.0, 1.0).await,
        Err(MapError::NotFound),
    ));
}

#[sqlx::test]
async fn add_connection_validates_and_dedupes(pool: PgPool) {
    let w = world(&pool).await;
    let a = add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();
    let b = add_system(&pool, w.owner, w.map_id, SYS_B, 1.0, 0.0, None)
        .await
        .unwrap();

    // Self-connection rejected.
    assert!(matches!(
        add_connection(
            &pool,
            w.owner,
            w.map_id,
            a.id,
            a.id,
            ConnectionType::Wormhole
        )
        .await,
        Err(MapError::Validation(_)),
    ));
    // Endpoint not on the map (an unplaced placement id) rejected.
    assert!(matches!(
        add_connection(
            &pool,
            w.owner,
            w.map_id,
            a.id,
            4242,
            ConnectionType::Wormhole
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    add_connection(
        &pool,
        w.owner,
        w.map_id,
        a.id,
        b.id,
        ConnectionType::Wormhole,
    )
    .await
    .unwrap();
    // Duplicate, reversed direction → Conflict (edges are unordered).
    assert!(matches!(
        add_connection(
            &pool,
            w.owner,
            w.map_id,
            b.id,
            a.id,
            ConnectionType::Stargate
        )
        .await,
        Err(MapError::Conflict(_)),
    ));
}

#[sqlx::test]
async fn remove_connection_clears_signature_link(pool: PgPool) {
    let w = world(&pool).await;
    let a = add_system(&pool, w.owner, w.map_id, SYS_A, 0.0, 0.0, None)
        .await
        .unwrap();
    let b = add_system(&pool, w.owner, w.map_id, SYS_B, 1.0, 0.0, None)
        .await
        .unwrap();
    let conn = add_connection(
        &pool,
        w.owner,
        w.map_id,
        a.id,
        b.id,
        ConnectionType::Wormhole,
    )
    .await
    .unwrap();

    // A wormhole signature linked to the connection.
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

    remove_connection(&pool, w.owner, w.map_id, conn.id)
        .await
        .unwrap();

    // The signature survives with its connection_id cleared.
    let (count, linked): (i64, i64) = (
        sqlx::query_scalar("select count(*) from signatures where map_id = $1")
            .bind(w.map_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        sqlx::query_scalar("select count(*) from signatures where connection_id is not null")
            .fetch_one(&pool)
            .await
            .unwrap(),
    );
    assert_eq!(count, 1);
    assert_eq!(linked, 0);

    assert!(matches!(
        remove_connection(&pool, w.owner, w.map_id, 4242).await,
        Err(MapError::NotFound),
    ));
}
