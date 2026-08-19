//! Access management: granting, changing, and revoking roles, with the privilege
//! ceiling and the at-least-one-owner invariant.

mod common;

use chrono::{Duration, Utc};
use common::{add_character, member_with_role, new_user, world};
use sqlx::PgPool;
use vector::maps::access::{
    RevokeAccess, SetAccess, TransferOwnership, effective_role, list_access, revoke_access,
    set_access, transfer_ownership,
};
use vector::maps::map::{GetMap, get_map};
use vector::maps::{Actor, MapError, Role, SubjectType};

#[sqlx::test]
async fn set_access_grants_and_then_changes_role_in_place(pool: PgPool) {
    let w = world(&pool).await;
    let user = new_user(&pool).await;
    add_character(&pool, user, 1002, 2002, None).await;

    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1002,
            role: Role::Member,
            expires_at: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        effective_role(&pool, w.map_id, user).await.unwrap(),
        Some(Role::Member)
    );

    // Re-granting the same subject updates the role rather than adding a row.
    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1002,
            role: Role::Manager,
            expires_at: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        effective_role(&pool, w.map_id, user).await.unwrap(),
        Some(Role::Manager)
    );
    let rows: i64 = sqlx::query_scalar(
        "select count(*) from map_access where map_id = $1 and subject_id = 1002",
    )
    .bind(w.map_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, 1, "re-grant must upsert, not duplicate");
}

#[sqlx::test]
async fn grant_respects_privilege_ceiling(pool: PgPool) {
    let w = world(&pool).await;
    let manager = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Manager).await;

    // A manager cannot grant a role above their own.
    assert!(matches!(
        set_access(
            &pool,
            manager,
            SetAccess {
                map_id: w.map_id,
                subject_type: SubjectType::Character,
                subject_id: 1003,
                role: Role::Owner,
                expires_at: None,
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));
    // But can grant up to manager.
    set_access(
        &pool,
        manager,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1003,
            role: Role::Manager,
            expires_at: None,
        },
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn owner_invariant_blocks_downgrade_and_revoke(pool: PgPool) {
    let w = world(&pool).await;

    // The sole owner cannot be downgraded...
    assert!(matches!(
        set_access(
            &pool,
            w.owner,
            SetAccess {
                map_id: w.map_id,
                subject_type: SubjectType::Character,
                subject_id: w.owner.character_id,
                role: Role::Manager,
                expires_at: None,
            }
        )
        .await,
        Err(MapError::LastOwner),
    ));
    // ...nor revoked.
    assert!(matches!(
        revoke_access(
            &pool,
            w.owner,
            RevokeAccess {
                map_id: w.map_id,
                subject_id: w.owner.character_id
            }
        )
        .await,
        Err(MapError::LastOwner),
    ));
    // The owner grant is still intact after the rejected attempts.
    assert_eq!(
        effective_role(&pool, w.map_id, w.owner.user_id)
            .await
            .unwrap(),
        Some(Role::Owner)
    );
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

    revoke_access(
        &pool,
        w.owner,
        RevokeAccess {
            map_id: w.map_id,
            subject_id: member.character_id,
        },
    )
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
        revoke_access(
            &pool,
            w.owner,
            RevokeAccess {
                map_id: w.map_id,
                subject_id: 4242
            }
        )
        .await,
        Err(MapError::NotFound),
    ));
}

#[sqlx::test]
async fn access_via_corporation_grant(pool: PgPool) {
    let w = world(&pool).await;

    // Grant a corporation; a character in that corp gains access, by corp not by id.
    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Corporation,
            subject_id: 5000,
            role: Role::Member,
            expires_at: None,
        },
    )
    .await
    .unwrap();
    let user = new_user(&pool).await;
    add_character(&pool, user, 1002, 5000, None).await;
    assert_eq!(
        effective_role(&pool, w.map_id, user).await.unwrap(),
        Some(Role::Member)
    );

    // A character in a different corp gets nothing.
    let other = new_user(&pool).await;
    add_character(&pool, other, 1003, 6000, None).await;
    assert_eq!(effective_role(&pool, w.map_id, other).await.unwrap(), None);
}

/// Access is granted per user (across all their characters), but tracking and waypoints
/// act as the *active* character. A user can therefore read a map through one character
/// while the one they are flying is not covered by any grant, which is what the map's
/// limited-access warning is about.
#[sqlx::test]
async fn map_view_flags_an_active_character_without_its_own_grant(pool: PgPool) {
    use vector::maps::Actor;
    use vector::maps::map::{GetMap, get_map};

    let w = world(&pool).await;
    let user = new_user(&pool).await;
    add_character(&pool, user, 1002, 2002, None).await;
    add_character(&pool, user, 1003, 3003, None).await;
    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1002,
            role: Role::Member,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let granted = Actor {
        user_id: user,
        character_id: 1002,
    };
    let view = get_map(&pool, granted, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.role, Role::Member);
    assert!(view.character_has_access);

    // The user's other character sees the same map at the same role, but is itself
    // outside every grant.
    let flying = Actor {
        user_id: user,
        character_id: 1003,
    };
    let view = get_map(&pool, flying, GetMap { map_id: w.map_id })
        .await
        .unwrap();
    assert_eq!(view.role, Role::Member);
    assert!(!view.character_has_access);
}

#[sqlx::test]
async fn a_grant_can_be_given_a_date_it_runs_out_on(pool: PgPool) {
    let w = world(&pool).await;
    let scout = new_user(&pool).await;
    add_character(&pool, scout, 1500, 2001, None).await;
    let scout_actor = Actor {
        user_id: scout,
        character_id: 1500,
    };

    // Access for the operation, not for ever.
    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1500,
            role: Role::Member,
            expires_at: Some(Some(Utc::now() + Duration::hours(4))),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        effective_role(&pool, w.map_id, scout).await.unwrap(),
        Some(Role::Member)
    );

    // Once it lapses the row is still there, and counts for nothing.
    sqlx::query(
        "update map_access set expires_at = now() - interval '1 minute' where subject_id = 1500",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(effective_role(&pool, w.map_id, scout).await.unwrap(), None);
    assert!(matches!(
        get_map(&pool, scout_actor, GetMap { map_id: w.map_id }).await,
        Err(MapError::NotFound),
    ));
    // And it is not offered as one of the map's grants either.
    let listed = list_access(&pool, w.owner, w.map_id).await.unwrap();
    assert!(listed.iter().all(|e| e.subject_id != 1500));

    // Taking the date off makes it a grant like any other again.
    set_access(
        &pool,
        w.owner,
        SetAccess {
            map_id: w.map_id,
            subject_type: SubjectType::Character,
            subject_id: 1500,
            role: Role::Member,
            expires_at: Some(None),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        effective_role(&pool, w.map_id, scout).await.unwrap(),
        Some(Role::Member)
    );
}

#[sqlx::test]
async fn ownership_is_handed_on_rather_than_granted(pool: PgPool) {
    let w = world(&pool).await;
    let mate = member_with_role(&pool, w.owner, w.map_id, 1010, 2001, Role::Manager).await;

    // Even the owner cannot make a second owner: there is one, and it moves.
    assert!(matches!(
        set_access(
            &pool,
            w.owner,
            SetAccess {
                map_id: w.map_id,
                subject_type: SubjectType::Character,
                subject_id: 1010,
                role: Role::Owner,
                expires_at: None,
            }
        )
        .await,
        Err(MapError::Validation(_)),
    ));

    // Only the owner may hand it on.
    assert!(matches!(
        transfer_ownership(
            &pool,
            mate,
            TransferOwnership {
                map_id: w.map_id,
                subject_id: 1010,
            }
        )
        .await,
        Err(MapError::Forbidden),
    ));

    // And only to somebody already on the map.
    assert!(matches!(
        transfer_ownership(
            &pool,
            w.owner,
            TransferOwnership {
                map_id: w.map_id,
                subject_id: 4444,
            }
        )
        .await,
        Err(MapError::NotFound),
    ));

    transfer_ownership(
        &pool,
        w.owner,
        TransferOwnership {
            map_id: w.map_id,
            subject_id: 1010,
        },
    )
    .await
    .unwrap();

    // The map has exactly one owner, and the old one stays on to keep running it.
    let entries = list_access(&pool, w.owner, w.map_id).await.unwrap();
    let owners: Vec<_> = entries.iter().filter(|e| e.role == Role::Owner).collect();
    assert_eq!(owners.len(), 1);
    assert_eq!(owners[0].subject_id, 1010);
    assert_eq!(
        entries
            .iter()
            .find(|e| e.subject_id == 1001)
            .map(|e| e.role),
        Some(Role::Manager)
    );
}
