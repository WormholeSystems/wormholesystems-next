//! The per-map navigation watchlist: CRUD, idempotent add, role gating.

mod common;

use common::{SYS_A, SYS_B, member_with_role, world};
use sqlx::PgPool;
use vector::maps::watchlist::{
    AddWatchlistEntry, RemoveWatchlistEntry, SetWatchlistPinned, add_watchlist_entry,
    list_watchlist, remove_watchlist_entry, set_watchlist_pinned,
};
use vector::maps::{MapError, Role};

#[sqlx::test]
async fn crud_and_gating(pool: PgPool) {
    let w = world(&pool).await;
    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;

    // Viewers can read but not mutate.
    let err = add_watchlist_entry(
        &pool,
        viewer,
        AddWatchlistEntry {
            map_id: w.map_id,
            solar_system_id: SYS_A,
        },
    )
    .await;
    assert!(matches!(err, Err(MapError::Forbidden)));

    let entry = add_watchlist_entry(
        &pool,
        w.owner,
        AddWatchlistEntry {
            map_id: w.map_id,
            solar_system_id: SYS_A,
        },
    )
    .await
    .unwrap();
    assert!(!entry.is_pinned);

    // Adding the same system again is idempotent (same row).
    let again = add_watchlist_entry(
        &pool,
        w.owner,
        AddWatchlistEntry {
            map_id: w.map_id,
            solar_system_id: SYS_A,
        },
    )
    .await
    .unwrap();
    assert_eq!(again.id, entry.id);

    add_watchlist_entry(
        &pool,
        w.owner,
        AddWatchlistEntry {
            map_id: w.map_id,
            solar_system_id: SYS_B,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        list_watchlist(&pool, viewer, w.map_id).await.unwrap().len(),
        2
    );

    let pinned = set_watchlist_pinned(
        &pool,
        w.owner,
        SetWatchlistPinned {
            map_id: w.map_id,
            entry_id: entry.id,
            value: true,
        },
    )
    .await
    .unwrap();
    assert!(pinned.is_pinned);

    remove_watchlist_entry(
        &pool,
        w.owner,
        RemoveWatchlistEntry {
            map_id: w.map_id,
            entry_id: entry.id,
        },
    )
    .await
    .unwrap();
    let remaining = list_watchlist(&pool, w.owner, w.map_id).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].solar_system_id, SYS_B);

    // Unknown entry → NotFound.
    let err = remove_watchlist_entry(
        &pool,
        w.owner,
        RemoveWatchlistEntry {
            map_id: w.map_id,
            entry_id: entry.id,
        },
    )
    .await;
    assert!(matches!(err, Err(MapError::NotFound)));
}
