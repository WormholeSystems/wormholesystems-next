use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

pub mod api;
pub mod components;
pub mod pages;

use api::{
    CharacterRef, CharacterStatus, active_character_status, current_character, my_characters,
    switch_character,
};
use pages::{HomePage, LoginPage, MapPage, MapsPage, ProfilePage};

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
    // Bumped each time the per-user socket receives an event; the navbar status resource
    // sources on it to refetch live.
    let status_version = RwSignal::new(0u32);
    provide_context(status_version);
    // Open the per-user heartbeat socket (client-only; a no-op / 401 when not signed in).
    Effect::new(move |_| open_user_socket(status_version));

    view! {
        <Stylesheet id="leptos" href="/pkg/vector.css" />
        <Title text="Vector" />
        <Router>
            <Nav />
            <main class="mx-auto max-w-5xl p-6">
                <Routes fallback=|| "Not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage />
                    <Route path=StaticSegment("login") view=LoginPage />
                    <Route path=StaticSegment("profile") view=ProfilePage />
                    <Route
                        path=StaticSegment("maps")
                        view=|| view! { <Protected><MapsPage /></Protected> }
                    />
                    <Route
                        path=(StaticSegment("maps"), ParamSegment("id"))
                        view=|| view! { <Protected><MapPage /></Protected> }
                    />
                </Routes>
            </main>
        </Router>
    }
}

/// Client-side gate: when a session-less client navigates (SPA) to a protected route, bounce
/// to the login page. Server-side full loads are gated earlier by the Axum `require_login`
/// middleware, so on the server this just renders the children for the (already-authed) user.
#[component]
fn Protected(children: ChildrenFn) -> impl IntoView {
    let account = Resource::new(|| (), |_| async move { current_character().await });

    view! {
        <Suspense fallback=|| view! { <p class="text-slate-500">"Loading…"</p> }>
            {move || {
                let children = children.clone();
                Suspend::new(async move {
                    match account.await {
                        Ok(Some(_)) => children().into_any(),
                        _ => {
                            redirect_to_login();
                            view! { <p class="text-slate-500">"Redirecting to log in…"</p> }
                                .into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}

#[cfg(feature = "hydrate")]
fn redirect_to_login() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href("/login");
    }
}

#[cfg(not(feature = "hydrate"))]
fn redirect_to_login() {}

/// Top bar: app links plus the auth state — signed-in character + switcher + logout, or a
/// log-in link. One resource + one Suspense (no nested async boundary), so SSR and
/// hydration render the same structure.
#[component]
fn Nav() -> impl IntoView {
    let account = Resource::new(
        || (),
        |_| async move {
            let character = current_character().await.ok().flatten();
            let characters = if character.is_some() {
                my_characters().await.unwrap_or_default()
            } else {
                Vec::new()
            };
            (character, characters)
        },
    );

    // Live status of the active character, refetched whenever the user socket pings.
    let status_version = expect_context::<RwSignal<u32>>();
    let status = Resource::new(
        move || status_version.get(),
        |_| async move { active_character_status().await.ok().flatten() },
    );

    view! {
        <nav class="flex items-center gap-4 border-b px-6 py-3">
            <a href="/" class="font-bold">"Vector"</a>
            <a href="/maps" class="text-sm text-blue-600 hover:underline">"Maps"</a>
            <span class="ml-auto flex items-center gap-3 text-sm">
                {move || status_badge(status.get().flatten())}
                <Suspense fallback=|| ()>
                    {move || Suspend::new(async move {
                        let (character, characters) = account.await;
                        match character {
                            Some(character) => view! {
                                <span class="text-slate-600">{character.name}</span>
                                {character_switcher(characters)}
                                <a
                                    href="/auth/logout"
                                    rel="external"
                                    class="text-blue-600 hover:underline"
                                >
                                    "Log out"
                                </a>
                            }
                            .into_any(),
                            None => view! {
                                <a
                                    href="/auth/login"
                                    rel="external"
                                    class="text-blue-600 hover:underline"
                                >
                                    "Log in"
                                </a>
                            }
                            .into_any(),
                        }
                    })}
                </Suspense>
            </span>
        </nav>
    }
}

/// The active character's live status: an online dot, current system, and ship. Empty when
/// not signed in or not yet tracked.
fn status_badge(status: Option<CharacterStatus>) -> AnyView {
    let Some(s) = status else {
        return ().into_any();
    };
    let dot = if s.online { "#22c55e" } else { "#94a3b8" };
    let system = s.solar_system.unwrap_or_else(|| "—".into());
    let ship = s.ship_type.unwrap_or_else(|| "—".into());
    view! {
        <span class="flex items-center gap-1.5 text-slate-500">
            <span
                class="inline-block w-2 h-2 rounded-full"
                style=format!("background:{dot}")
            ></span>
            <span>{system}</span>
            <span class="text-slate-400">"·"</span>
            <span>{ship}</span>
        </span>
    }
    .into_any()
}

/// A `<select>` to change the active character, shown only when the user has more than one.
/// Plain markup (no Suspense of its own) so it hydrates inside the Nav's boundary. Switching
/// is a session change, so we reload to refetch everything as the new character.
fn character_switcher(characters: Vec<CharacterRef>) -> AnyView {
    if characters.len() < 2 {
        return ().into_any();
    }
    view! {
        <select
            class="border rounded text-sm px-1 py-0.5"
            on:change=move |ev| {
                if let Ok(id) = event_target_value(&ev).parse::<i64>() {
                    spawn_local(async move {
                        if switch_character(id).await.is_ok() {
                            reload_page();
                        }
                    });
                }
            }
        >
            {characters
                .into_iter()
                .map(|c| {
                    view! {
                        <option value=c.character_id.to_string() selected=c.is_active>
                            {c.name}
                        </option>
                    }
                })
                .collect_view()}
        </select>
    }
    .into_any()
}

#[cfg(feature = "hydrate")]
fn reload_page() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

#[cfg(not(feature = "hydrate"))]
fn reload_page() {}

/// Open the per-user heartbeat WebSocket and keep it alive for the page's lifetime. The
/// browser auto-replies to the server's pings, which refreshes `last_active_at`.
#[cfg(feature = "hydrate")]
fn open_user_socket(version: RwSignal<u32>) {
    use futures::StreamExt;
    use gloo_net::websocket::futures::WebSocket;

    let Some(window) = web_sys::window() else {
        return;
    };
    let location = window.location();
    let scheme = match location.protocol().as_deref() {
        Ok("https:") => "wss",
        _ => "ws",
    };
    let host = location.host().unwrap_or_default();
    let Ok(mut socket) = WebSocket::open(&format!("{scheme}://{host}/ws/user")) else {
        return;
    };
    leptos::task::spawn_local(async move {
        // Each event means "your status changed" — bump the version so the navbar refetches.
        while let Some(Ok(_)) = socket.next().await {
            version.update(|n| *n += 1);
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn open_user_socket(_version: RwSignal<u32>) {}
