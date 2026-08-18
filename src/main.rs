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

    // `vector discord-register` uploads the slash commands, then exits. Run it once per
    // deploy that changes them; Discord takes a few minutes to roll a change out.
    if args.iter().any(|a| a == "discord-register") {
        dotenvy::dotenv().ok();
        let application_id =
            std::env::var("DISCORD_APPLICATION_ID").expect("DISCORD_APPLICATION_ID not set");
        let token = std::env::var("DISCORD_BOT_TOKEN").expect("DISCORD_BOT_TOKEN not set");
        match vector::discord::commands::register(&application_id, &token).await {
            Ok(()) => println!("registered the /vector command."),
            Err(err) => panic!("could not register commands: {err}"),
        }
        return;
    }

    // `vector threat-analysis` recomputes every wormhole system's threat from the killmails
    // already stored, then exits. The server does this daily and after a backfill; this is
    // for when you want the numbers now.
    if args.iter().any(|a| a == "threat-analysis") {
        dotenvy::dotenv().ok();
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
        let db = vector::db::connect(&url).await.expect("db connect failed");
        vector::killmails::analyze(&db, &vector::esi::EsiClient::new())
            .await
            .expect("threat analysis failed");
        println!("threat analysis complete.");
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

    // Background: mirror the raidable skyhooks CCP is currently advertising.
    vector::skyhooks::start(db.clone(), esi.clone(), server.clone());

    // Background: Discord alerts. One runtime shared by everything that can fire one; the
    // stargate graph it holds is the expensive part and is the same for all of them.
    let alerts = match vector::alerts::Runtime::load(&db).await {
        Ok(runtime) => Some(Arc::new(runtime)),
        Err(err) => {
            eprintln!("alerts disabled, could not load the stargate graph: {err}");
            None
        }
    };
    if let Some(alerts) = alerts.clone() {
        vector::alerts::start(db.clone(), hub.clone(), alerts);
    }

    // Background: killmail ingest + daily threat analysis (gated by ZKB_LISTEN=1).
    vector::killmails::start(db.clone(), esi.clone(), hub.clone(), alerts);

    // Background: purge stale signatures (legacy expiry: 3d wormholes, 7d sites).
    vector::maps::signatures::start_expiry(db.clone(), hub.clone());

    // Background: prune unclaimed connection-jump observations.
    vector::maps::jumps::start_prune(db.clone());

    // Background: drop command-journal entries past the undo retention window.
    vector::maps::events_log::start_purge(db.clone());

    let state = AppState {
        auth,
        discord: config.discord.clone().map(Arc::new),
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
        // Discord: linking an account, and the bot's interaction endpoint.
        .route("/discord/connect", get(vector::discord::link::connect))
        .route("/discord/callback", get(vector::discord::link::callback))
        .route(
            "/discord/interactions",
            axum::routing::post(vector::discord::interactions::handle),
        )
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
