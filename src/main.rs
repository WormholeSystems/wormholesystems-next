#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    // `vector seed` populates the reference tables from the SDE + data/static, then exits.
    if std::env::args().any(|a| a == "seed") {
        vector::seed::run().await.expect("seeding failed");
        return;
    }

    use std::sync::Arc;

    use axum::Router;
    use axum::routing::get;
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use vector::app::{App, shell};
    use vector::auth::{self, AppState, Auth};
    use vector::config::Config;
    use vector::esi::{EsiClient, Sso};

    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // Config, the database, and SSO are all essential — fail fast if any is unavailable.
    let config = Config::from_env()
        .expect("missing configuration — copy .env.example to .env and fill in the variables");
    let db = vector::db::connect(&config.database_url).await.expect(
        "could not connect to Postgres — is it running? start it with `docker compose up -d`",
    );
    // Keep the reference tables in sync with the bundled SDE. Runs only on first boot
    // or when data/sde holds a newer build; otherwise it's a single cheap query.
    match vector::seed::ensure_seeded(&db).await {
        Ok(true) => log!("seeded reference tables from the SDE"),
        Ok(false) => {}
        Err(e) => panic!("could not seed the SDE reference tables: {e}"),
    }
    let sso = Arc::new(
        Sso::discover(reqwest::Client::new(), config.sso)
            .await
            .expect("could not reach the EVE SSO — check your network connection"),
    );
    let esi = EsiClient::new();
    let auth = Arc::new(Auth::new(sso.clone(), esi.clone()));

    let hub = vector::maps::MapHub::new();
    let user_hub = vector::user_channel::UserHub::new();

    // Background: poll live character status for active users (no queue; in-process). Pings
    // each user's private channel when their character's status changes.
    vector::tracking::start(db.clone(), sso.clone(), esi.clone(), user_hub.clone());

    let state = AppState {
        leptos_options: leptos_options.clone(),
        auth,
        db: db.clone(),
        hub: hub.clone(),
        user_hub: user_hub.clone(),
    };

    // Server functions reach the DB + event hub through Leptos context. The same context must
    // be provided on every path that renders <App> — both the page routes and the error/404
    // fallback, which also renders <Nav> (and so calls DB-backed server fns during SSR).
    let provide_app_context = {
        let db = db.clone();
        let hub = hub.clone();
        move || {
            provide_context(db.clone());
            provide_context(hub.clone());
        }
    };

    let app = Router::new()
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", get(auth::logout))
        // Realtime: per-map event stream + the per-user private channel / activity heartbeat.
        .route("/ws/map/{map_id}", get(vector::app::api::map_ws))
        .route("/ws/user", get(vector::app::api::user_ws))
        .leptos_routes_with_context(
            &state,
            routes,
            provide_app_context.clone(),
            {
                let leptos_options = leptos_options.clone();
                move || shell(leptos_options.clone())
            },
        )
        .fallback(leptos_axum::file_and_error_handler_with_context::<AppState, _>(
            provide_app_context.clone(),
            shell,
        ))
        // Gate protected pages (/maps...) before rendering; redirects unauthenticated loads.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_login,
        ))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    log!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
fn main() {}
