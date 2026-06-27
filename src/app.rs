use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::StaticSegment;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::use_query_map;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <AutoReload options=options.clone() />
                <HydrationScripts options />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/vector.css" />
        <Title text="Vector" />
        <Router>
            <main>
                <Routes fallback=|| "Not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("login") view=LoginPage />
                    <Route path=StaticSegment("profile") view=ProfilePage />
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <h1>"Vector"</h1>
        <p>"Wormhole mapping for EVE Online — under construction."</p>
        <p><a href="/login">"Log in"</a></p>
    }
}

#[component]
fn LoginPage() -> impl IntoView {
    // `rel="external"` stops the Leptos router from intercepting the click as an
    // in-app route — we want a full navigation to the server route, which 302s to EVE.
    view! {
        <h1>"Log in"</h1>
        <a href="/auth/login" rel="external">
            "Log in with EVE Online"
        </a>
    }
}

#[component]
fn ProfilePage() -> impl IntoView {
    let query = use_query_map();
    let param = move |key: &str| query.read().get(key);

    view! {
        <h1>"Profile"</h1>
        {move || match param("name") {
            None => view! { <p>"Not logged in. " <a href="/login">"Log in"</a></p> }.into_any(),
            Some(name) => view! {
                <p>"Login succeeded — logged in as " <strong>{name}</strong> "."</p>
                <h2>"Affiliations"</h2>
                <ul>
                    {move || param("corporation").map(|c| view! { <li>"Corporation: " {c}</li> })}
                    {move || param("alliance").map(|a| view! { <li>"Alliance: " {a}</li> })}
                    {move || param("faction").map(|f| view! { <li>"Faction: " {f}</li> })}
                </ul>
                <h2>"Granted scopes"</h2>
                {move || match param("scopes") {
                    None => view! { <p>"None."</p> }.into_any(),
                    Some(scopes) => {
                        let items: Vec<String> = scopes.split(' ').map(str::to_string).collect();
                        view! {
                            <ul>
                                {items.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                            </ul>
                        }
                        .into_any()
                    }
                }}
            }
            .into_any(),
        }}
    }
}
