//! The SSO login → identity resolution rules from authentication.md: account reuse,
//! character-transfer (owner-hash change) reassignment, linking, preferred selection, and
//! the session lifecycle. These are pure DB logic, so they're driven directly with a pool.

use sqlx::PgPool;
use vector::esi::jwt::Claims;
use vector::session::{
    Entity, actor_for_session, create_session, delete_session, persist_identity,
    set_active_character,
};

fn claims(character_id: i64, owner_hash: &str) -> Claims {
    Claims {
        character_id,
        name: format!("Pilot {character_id}"),
        owner_hash: owner_hash.into(),
        scopes: vec![],
    }
}

fn corp(id: i64) -> Entity {
    Entity {
        id,
        name: format!("Corp {id}"),
        ticker: "CRP".into(),
    }
}

async fn character_user(pool: &PgPool, character_id: i64) -> i64 {
    sqlx::query_scalar("select user_id from characters where id = $1")
        .bind(character_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn is_preferred(pool: &PgPool, character_id: i64) -> bool {
    sqlx::query_scalar("select is_preferred from characters where id = $1")
        .bind(character_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn new_character_creates_a_preferred_character_and_caches_its_corp(pool: PgPool) {
    let user = persist_identity(&pool, &claims(100, "h1"), corp(2001), None, None)
        .await
        .unwrap();

    assert_eq!(character_user(&pool, 100).await, user);
    assert!(
        is_preferred(&pool, 100).await,
        "first character is preferred"
    );

    // The corp entity was cached (deferred FK target).
    let corp_count: i64 = sqlx::query_scalar("select count(*) from corporations where id = 2001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(corp_count, 1);
}

#[sqlx::test]
async fn returning_login_reuses_the_account_and_refreshes_corp(pool: PgPool) {
    let user = persist_identity(&pool, &claims(100, "h1"), corp(2001), None, None)
        .await
        .unwrap();
    // Same owner hash, different corp → same user, corp updated.
    let again = persist_identity(&pool, &claims(100, "h1"), corp(2002), None, None)
        .await
        .unwrap();

    assert_eq!(user, again, "returning login keeps the same account");
    let (uid, corp_id): (i64, i64) =
        sqlx::query_as("select user_id, corporation_id from characters where id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(uid, user);
    assert_eq!(corp_id, 2002, "corp refreshed on login");
    let users: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(users, 1, "no extra account created");
}

#[sqlx::test]
async fn owner_hash_change_reassigns_the_character_to_a_new_account(pool: PgPool) {
    let old = persist_identity(&pool, &claims(100, "h1"), corp(2001), None, None)
        .await
        .unwrap();
    // The character was transferred (new owner hash) → must land on a fresh account.
    let new = persist_identity(&pool, &claims(100, "h2"), corp(2001), None, None)
        .await
        .unwrap();

    assert_ne!(old, new, "transfer must not sign into the previous account");
    assert_eq!(character_user(&pool, 100).await, new);
    let owner: String = sqlx::query_scalar("select owner_hash from characters where id = 100")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, "h2");
    assert!(
        is_preferred(&pool, 100).await,
        "first character of the new account is preferred"
    );
}

#[sqlx::test]
async fn linking_attaches_a_character_without_making_it_preferred(pool: PgPool) {
    let user = persist_identity(&pool, &claims(100, "h1"), corp(2001), None, None)
        .await
        .unwrap();
    // A second character linked to the signed-in user.
    let linked = persist_identity(&pool, &claims(200, "h2"), corp(2001), None, Some(user))
        .await
        .unwrap();

    assert_eq!(linked, user, "linked character joins the existing account");
    assert_eq!(character_user(&pool, 200).await, user);
    assert!(
        is_preferred(&pool, 100).await,
        "the first character stays preferred"
    );
    assert!(
        !is_preferred(&pool, 200).await,
        "a linked second character is not preferred"
    );
}

#[sqlx::test]
async fn session_lifecycle_and_character_switch(pool: PgPool) {
    let user = persist_identity(&pool, &claims(100, "h1"), corp(2001), None, None)
        .await
        .unwrap();
    persist_identity(&pool, &claims(200, "h2"), corp(2001), None, Some(user))
        .await
        .unwrap();
    // A character on a *different* account, to test the switch guard.
    let other = persist_identity(&pool, &claims(300, "h3"), corp(2001), None, None)
        .await
        .unwrap();
    assert_ne!(other, user);

    let session = create_session(&pool, user, 100).await.unwrap();
    let actor = actor_for_session(&pool, &session).await.unwrap().unwrap();
    assert_eq!((actor.user_id, actor.character_id), (user, 100));

    // Switch to another of the user's own characters → allowed.
    assert!(set_active_character(&pool, &session, 200).await.unwrap());
    let actor = actor_for_session(&pool, &session).await.unwrap().unwrap();
    assert_eq!(actor.character_id, 200);

    // Switching to a character the user doesn't own → rejected, unchanged.
    assert!(!set_active_character(&pool, &session, 300).await.unwrap());
    assert_eq!(
        actor_for_session(&pool, &session)
            .await
            .unwrap()
            .unwrap()
            .character_id,
        200
    );

    // Logout invalidates the session.
    delete_session(&pool, &session).await.unwrap();
    assert!(actor_for_session(&pool, &session).await.unwrap().is_none());
}
