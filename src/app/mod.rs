use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

pub mod api;
pub mod components;
pub mod pages;

use api::{current_character, my_characters, switch_character};
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
                    <Route path=StaticSegment("maps") view=MapsPage />
                    <Route path=(StaticSegment("maps"), ParamSegment("id")) view=MapPage />
                </Routes>
            </main>
        </Router>
    }
}

/// Top bar: app links plus the auth state — signed-in character + switcher + logout, or a
/// log-in link.
#[component]
fn Nav() -> impl IntoView {
    let account = Resource::new(|| (), |_| async move { current_character().await });

    view! {
        <nav class="flex items-center gap-4 border-b px-6 py-3">
            <a href="/" class="font-bold">"Vector"</a>
            <a href="/maps" class="text-sm text-blue-600 hover:underline">"Maps"</a>
            <span class="ml-auto flex items-center gap-3 text-sm">
                <Transition fallback=|| ()>
                    {move || Suspend::new(async move {
                        match account.await {
                            Ok(Some(character)) => view! {
                                <span class="text-slate-600">{character.name}</span>
                                <CharacterSwitcher />
                                <a
                                    href="/auth/logout"
                                    rel="external"
                                    class="text-blue-600 hover:underline"
                                >
                                    "Log out"
                                </a>
                            }
                            .into_any(),
                            _ => view! {
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
                </Transition>
            </span>
        </nav>
    }
}

/// A `<select>` to change the active character; only shown when the user has more than one.
/// Switching is a session change, so we reload to refetch everything as the new character.
#[component]
fn CharacterSwitcher() -> impl IntoView {
    let characters = Resource::new(|| (), |_| async move { my_characters().await });

    view! {
        <Transition fallback=|| ()>
            {move || Suspend::new(async move {
                let list = characters.await.unwrap_or_default();
                if list.len() < 2 {
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
                        {list
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
            })}
        </Transition>
    }
}

#[cfg(feature = "hydrate")]
fn reload_page() {
    if let Some(window) = web_sys::window() {
        let _ = window.location().reload();
    }
}

#[cfg(not(feature = "hydrate"))]
fn reload_page() {}
