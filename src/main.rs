#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
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

    // Build the SSO client if credentials are configured; otherwise run without auth
    // (the /auth routes then return 503 until EVE_* are set in .env).
    let auth = match Config::from_env() {
        Ok(config) => match Sso::discover(reqwest::Client::new(), config.sso).await {
            Ok(sso) => Some(Arc::new(Auth::new(sso, EsiClient::new()))),
            Err(e) => {
                log!("SSO discovery failed, auth disabled: {e}");
                None
            }
        },
        Err(e) => {
            log!("SSO not configured, auth disabled: {e}");
            None
        }
    };

    let state = AppState {
        leptos_options: leptos_options.clone(),
        auth,
    };

    let app = Router::new()
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .leptos_routes(&state, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    log!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
fn main() {}
