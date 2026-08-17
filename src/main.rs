#[tokio::main]
async fn main() {
    // `vector seed` populates the reference tables from the SDE + data/static, then exits.
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "seed") {
        vector::seed::run().await.expect("seeding failed");
        return;
    }
    // `vector killmails-backfill [days]` imports EVE Ref daily archives, then exits.
    if let Some(pos) = args.iter().position(|a| a == "killmails-backfill") {
        let days: u32 = args.get(pos + 1).and_then(|d| d.parse().ok()).unwrap_or(30);
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
        let db = vector::db::connect(&url).await.expect("db connect failed");
        vector::killmails::backfill(&db, &vector::esi::EsiClient::new(), days)
            .await
            .expect("backfill failed");
        return;
    }

    use std::sync::Arc;

    use axum::Router;
    use axum::routing::get;
    use vector::auth::{self, AppState, Auth};
    use vector::config::Config;
    use vector::esi::{EsiClient, Sso};

    // Config, the database, and SSO are all essential — fail fast if any is unavailable.
    let config = Config::from_env()
        .expect("missing configuration — copy .env.example to .env and fill in the variables");
    let db = vector::db::connect(&config.database_url).await.expect(
        "could not connect to Postgres — is it running? start it with `docker compose up -d`",
    );
    // Keep the reference tables in sync with the bundled SDE. Runs only on first boot
    // or when data/sde holds a newer build; otherwise it's a single cheap query.
    match vector::seed::ensure_seeded(&db).await {
        Ok(true) => println!("seeded reference tables from the SDE"),
        Ok(false) => {}
        Err(e) => panic!("could not seed the SDE reference tables: {e}"),
    }
    let sso = Arc::new(
        Sso::discover(reqwest::Client::new(), config.sso)
            .await
            .expect("could not reach the EVE SSO — check your network connection"),
    );
    if config.esi_base_url != vector::esi::BASE_URL {
        println!("ESI: {}", config.esi_base_url);
    }
    let esi = EsiClient::with_config(
        reqwest::Client::new(),
        &config.esi_base_url,
        vector::esi::COMPATIBILITY_DATE,
    );
    let auth = Arc::new(Auth::new(sso.clone(), esi.clone()));

    let hub = vector::maps::MapHub::new();
    let user_hub = vector::user_channel::UserHub::new();

    // Background: is Tranquility up? Everything below that talks to ESI gates on it, so it
    // is started first.
    let server = vector::server_status::start(db.clone(), esi.clone(), user_hub.clone());

    // Background: poll live character status for active users (no queue; in-process). Pings
    // each user's private channel when their character's status changes.
    vector::tracking::start(
        db.clone(),
        sso.clone(),
        esi.clone(),
        user_hub.clone(),
        hub.clone(),
        server.clone(),
    );

    // Background: keep sovereignty (and its alliance/corp entities) current for map display.
    vector::sovereignty::start(db.clone(), esi.clone(), server.clone());

    // Background: killmail ingest + daily threat analysis (gated by ZKB_LISTEN=1).
    vector::killmails::start(db.clone(), esi.clone());

    // Background: purge stale signatures (legacy expiry: 3d wormholes, 7d sites).
    vector::maps::signatures::start_expiry(db.clone(), hub.clone());

    // Background: prune unclaimed connection-jump observations.
    vector::maps::jumps::start_prune(db.clone());

    // Background: drop command-journal entries past the undo retention window.
    vector::maps::events_log::start_purge(db.clone());

    let state = AppState {
        auth,
        db,
        hub,
        user_hub,
        grid: config.grid,
        server,
    };

    let app = Router::new()
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", get(auth::logout))
        // Realtime: per-map event stream + the per-user private channel / activity heartbeat.
        .route("/ws/map/{map_id}", get(vector::api::ws::map_ws))
        .route("/ws/user", get(vector::api::ws::user_ws))
        // The JSON API for the SvelteKit frontend.
        .merge(vector::api::router())
        .with_state(state);

    let addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("listening on http://{addr}");
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
