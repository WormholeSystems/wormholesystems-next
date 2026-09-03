//! Per-map user settings over HTTP: the parts a bad body has to be refused on.

mod common;

use axum::http::StatusCode;
use common::app::{app, get, request_json, session_cookie};
use common::world;
use serde_json::json;
use sqlx::PgPool;

/// Only the caller's own characters can map for them: a stranger's id, or one that does
/// not exist, is refused rather than stored.
#[sqlx::test]
async fn tracked_pilots_must_be_the_callers_own(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let path = format!("/api/maps/{}/settings/user", w.map_id);

    let refused = request_json(
        app(&pool),
        "POST",
        &path,
        Some(&cookie),
        json!({ "tracked_character_ids": [w.owner.character_id, 9999] }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);

    let saved = request_json(
        app(&pool),
        "POST",
        &path,
        Some(&cookie),
        json!({ "tracked_character_ids": [w.owner.character_id] }),
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK);
    assert_eq!(
        saved.body["tracked_character_ids"],
        json!([w.owner.character_id])
    );

    let read = get(app(&pool), &path, Some(&cookie)).await;
    assert_eq!(
        read.body["tracked_character_ids"],
        json!([w.owner.character_id])
    );

    // Clearing the set is a real value, not an absent field.
    let cleared = request_json(
        app(&pool),
        "POST",
        &path,
        Some(&cookie),
        json!({ "tracked_character_ids": [] }),
    )
    .await;
    assert_eq!(cleared.body["tracked_character_ids"], json!([]));
}
