//! Personal access tokens over HTTP: minting, the one-time secret, bearer authentication
//! on the ordinary API and the v1 API, and the ways a token stops working.

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::app::{TestResponse, app, get, request_json, session_cookie};
use common::{SYS_A, SYS_B, member_with_role, world};
use serde_json::json;
use sqlx::PgPool;
use tower::ServiceExt;
use wormholesystems::maps::Role;

async fn send(app: Router, req: Request<Body>) -> TestResponse {
    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    TestResponse { status, body }
}

async fn bearer_get(app: Router, path: &str, token: &str) -> TestResponse {
    let req = Request::builder()
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    send(app, req).await
}

async fn bearer_json(
    app: Router,
    method: &str,
    path: &str,
    token: &str,
    body: serde_json::Value,
) -> TestResponse {
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

async fn mint(pool: &PgPool, cookie: &str, name: &str) -> (i64, String) {
    let created = request_json(
        app(pool),
        "POST",
        "/api/me/tokens",
        Some(cookie),
        json!({ "name": name }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{:?}", created.body);
    (
        created.body["token"]["id"].as_i64().unwrap(),
        created.body["plaintext"].as_str().unwrap().to_string(),
    )
}

#[sqlx::test]
async fn the_secret_is_shown_once_and_only_its_hash_is_kept(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;

    let (id, plaintext) = mint(&pool, &cookie, "my script").await;
    assert!(plaintext.starts_with("wst_"));

    let stored: String =
        sqlx::query_scalar("select token_hash from personal_access_tokens where id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored, plaintext);
    assert_eq!(stored, wormholesystems::tokens::hash(&plaintext));

    let listed = get(app(&pool), "/api/me/tokens", Some(&cookie)).await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.body[0]["name"], "my script");
    assert!(listed.body[0].get("plaintext").is_none());
    assert!(listed.body[0].get("token_hash").is_none());
    assert!(listed.body[0]["last_used_at"].is_null());
}

#[sqlx::test]
async fn a_blank_name_is_refused(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let refused = request_json(
        app(&pool),
        "POST",
        "/api/me/tokens",
        Some(&cookie),
        json!({ "name": "  " }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn a_bearer_token_acts_as_its_user(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let (id, plaintext) = mint(&pool, &cookie, "script").await;

    let me = bearer_get(app(&pool), "/api/me", &plaintext).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["character_id"], w.owner.character_id);

    let maps = bearer_get(app(&pool), "/api/v1/maps", &plaintext).await;
    assert_eq!(maps.status, StatusCode::OK);
    assert_eq!(maps.body["data"][0]["id"], w.map_id);
    assert_eq!(maps.body["data"][0]["name"], "Chain");
    assert_eq!(maps.body["data"][0]["owner"]["id"], w.owner.character_id);

    let used: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select last_used_at from personal_access_tokens where id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(used.is_some());
}

#[sqlx::test]
async fn a_bad_token_is_refused_rather_than_treated_as_a_guest(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;

    let wrong = bearer_get(app(&pool), "/api/me", "wst_nonsense").await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);

    // A public map is readable by a guest, but not by a client waving a bad token.
    sqlx::query("update maps set is_public = true where id = $1")
        .bind(w.map_id)
        .execute(&pool)
        .await
        .unwrap();
    let path = format!("/api/maps/{}", w.map_id);
    let guest = get(app(&pool), &path, None).await;
    assert_eq!(guest.status, StatusCode::OK);
    let bad = bearer_get(app(&pool), &path, "wst_nonsense").await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);

    let (id, plaintext) = mint(&pool, &cookie, "script").await;
    assert_eq!(
        bearer_get(app(&pool), "/api/me", &plaintext).await.status,
        StatusCode::OK
    );

    sqlx::query(
        "update personal_access_tokens set expires_at = now() - interval '1 minute' where id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    let expired = bearer_get(app(&pool), "/api/me", &plaintext).await;
    assert_eq!(expired.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn revoking_locks_the_token_out_and_only_the_owner_may_revoke(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let (id, plaintext) = mint(&pool, &cookie, "script").await;

    let stranger = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;
    let stranger_cookie = session_cookie(&pool, stranger).await;
    let path = format!("/api/me/tokens/{id}");
    let not_theirs = request_json(
        app(&pool),
        "DELETE",
        &path,
        Some(&stranger_cookie),
        json!({}),
    )
    .await;
    assert_eq!(not_theirs.status, StatusCode::NOT_FOUND);
    assert_eq!(
        bearer_get(app(&pool), "/api/me", &plaintext).await.status,
        StatusCode::OK
    );

    let revoked = request_json(app(&pool), "DELETE", &path, Some(&cookie), json!({})).await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert_eq!(
        bearer_get(app(&pool), "/api/me", &plaintext).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn tokens_are_managed_from_a_session_only(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let (_, plaintext) = mint(&pool, &cookie, "script").await;

    let listed = bearer_get(app(&pool), "/api/me/tokens", &plaintext).await;
    assert_eq!(listed.status, StatusCode::UNAUTHORIZED);
    let minted = bearer_json(
        app(&pool),
        "POST",
        "/api/me/tokens",
        &plaintext,
        json!({ "name": "another" }),
    )
    .await;
    assert_eq!(minted.status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn expiry_is_days_from_now(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let created = request_json(
        app(&pool),
        "POST",
        "/api/me/tokens",
        Some(&cookie),
        json!({ "name": "short lived", "expires_in_days": 30 }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    let expires: chrono::DateTime<chrono::Utc> =
        serde_json::from_value(created.body["token"]["expires_at"].clone()).unwrap();
    let days = (expires - chrono::Utc::now()).num_days();
    assert!((29..=30).contains(&days), "{days}");

    let past = request_json(
        app(&pool),
        "POST",
        "/api/me/tokens",
        Some(&cookie),
        json!({ "name": "already dead", "expires_in_days": 0 }),
    )
    .await;
    assert_eq!(past.status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn the_v1_api_places_edits_and_removes_a_system(pool: PgPool) {
    let w = world(&pool).await;
    let cookie = session_cookie(&pool, w.owner).await;
    let (_, token) = mint(&pool, &cookie, "mapper").await;

    let created = bearer_json(
        app(&pool),
        "POST",
        "/api/v1/map-solarsystems",
        &token,
        json!({
            "map_id": w.map_id,
            "solarsystem_id": SYS_A,
            "position_x": 100,
            "position_y": 100,
            "alias": "HOME",
            "status": "friendly",
            "pinned": true
        }),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
    let placement = created.body["data"]["id"].as_i64().unwrap();

    let path = format!("/api/v1/map-solarsystems/{placement}");
    let shown = bearer_get(app(&pool), &path, &token).await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(shown.body["data"]["alias"], "HOME");
    assert_eq!(shown.body["data"]["status"], "friendly");
    assert_eq!(shown.body["data"]["is_pinned"], true);
    assert_eq!(shown.body["data"]["solarsystem_id"], SYS_A);
    assert_eq!(shown.body["data"]["position"]["x"], 100.0);

    let updated = bearer_json(
        app(&pool),
        "PUT",
        &path,
        &token,
        json!({ "alias": "1", "notes": "staging", "status": "hostile", "pinned": false }),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{:?}", updated.body);
    let shown = bearer_get(app(&pool), &path, &token).await;
    assert_eq!(shown.body["data"]["alias"], "1");
    assert_eq!(shown.body["data"]["notes"], "staging");
    assert_eq!(shown.body["data"]["status"], "hostile");
    assert_eq!(shown.body["data"]["is_pinned"], false);

    let half = bearer_json(
        app(&pool),
        "PUT",
        &path,
        &token,
        json!({ "position_x": 200 }),
    )
    .await;
    assert_eq!(half.status, StatusCode::BAD_REQUEST);

    let map = bearer_get(app(&pool), &format!("/api/v1/maps/{}", w.map_id), &token).await;
    assert_eq!(map.body["data"]["map_solarsystems"][0]["id"], placement);
    assert_eq!(
        map.body["data"]["map_solarsystems"][0]["signatures_count"],
        0
    );

    let deleted = bearer_json(app(&pool), "DELETE", &path, &token, json!({})).await;
    assert_eq!(deleted.status, StatusCode::OK);
    let gone = bearer_get(app(&pool), &path, &token).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn the_v1_api_keeps_the_map_rules(pool: PgPool) {
    let w = world(&pool).await;
    let owner_cookie = session_cookie(&pool, w.owner).await;
    let placed = request_json(
        app(&pool),
        "POST",
        &format!("/api/maps/{}/systems/add", w.map_id),
        Some(&owner_cookie),
        json!({ "map_id": w.map_id, "solar_system_id": SYS_B, "x": 100, "y": 100 }),
    )
    .await;
    let placement = placed.body["id"].as_i64().unwrap();
    request_json(
        app(&pool),
        "POST",
        &format!("/api/maps/{}/systems/set-notes", w.map_id),
        Some(&owner_cookie),
        json!({ "map_id": w.map_id, "map_solar_system_id": placement, "notes": "secret" }),
    )
    .await;

    let viewer = member_with_role(&pool, w.owner, w.map_id, 1002, 2002, Role::Viewer).await;
    let viewer_cookie = session_cookie(&pool, viewer).await;
    let (_, token) = mint(&pool, &viewer_cookie, "viewer script").await;

    let path = format!("/api/v1/map-solarsystems/{placement}");
    let shown = bearer_get(app(&pool), &path, &token).await;
    assert_eq!(shown.status, StatusCode::OK);
    assert!(shown.body["data"]["notes"].is_null());

    let edit = bearer_json(app(&pool), "PUT", &path, &token, json!({ "alias": "X" })).await;
    assert_eq!(edit.status, StatusCode::FORBIDDEN);
    let rename = bearer_json(
        app(&pool),
        "PUT",
        &format!("/api/v1/maps/{}", w.map_id),
        &token,
        json!({ "name": "Mine now" }),
    )
    .await;
    assert_eq!(rename.status, StatusCode::FORBIDDEN);

    let outsider = common::new_user(&pool).await;
    common::add_character(&pool, outsider, 1003, 2003, None).await;
    let outsider_cookie = session_cookie(
        &pool,
        wormholesystems::maps::Actor {
            user_id: outsider,
            character_id: 1003,
        },
    )
    .await;
    let (_, outsider_token) = mint(&pool, &outsider_cookie, "outsider").await;
    let hidden = bearer_get(app(&pool), &path, &outsider_token).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let none = bearer_get(app(&pool), "/api/v1/maps", &outsider_token).await;
    assert_eq!(none.body["data"].as_array().unwrap().len(), 0);
}

#[sqlx::test]
async fn sovereignty_is_public_and_keyed_by_system(pool: PgPool) {
    common::seed_universe(&pool).await;
    sqlx::query("insert into alliances (id, name, ticker) values (99000001, 'Holders', 'HOLD')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into system_sovereignty (solar_system_id, alliance_id) values ($1, 99000001)",
    )
    .bind(SYS_A)
    .execute(&pool)
    .await
    .unwrap();

    let sov = get(app(&pool), "/api/v1/sovereignties", None).await;
    assert_eq!(sov.status, StatusCode::OK);
    let entry = &sov.body[SYS_A.to_string()];
    assert_eq!(entry["id"], SYS_A);
    assert_eq!(entry["alliance"]["ticker"], "HOLD");
    assert!(entry.get("corporation").is_none());
}
