use icons::{LogOut, Map, Plus, Trash2};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::{ParamSegment, StaticSegment};

pub mod api;
pub mod components;
pub mod pages;

use crate::app::components::{CharacterImage, TypeImage};
use crate::components::hooks::use_theme_mode::ThemeMode;
use crate::components::ui::dropdown_menu::{
    DropdownMenu, DropdownMenuAlign, DropdownMenuContent, DropdownMenuTrigger,
};
use crate::components::ui::theme_toggle::ThemeToggle;
use api::{
    CharacterRef, CharacterStatus, CharacterSummary, active_character_status, current_character,
    my_characters, remove_character, switch_character,
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
                // Apply the saved theme before first paint to avoid a flash.
                <script>
                    {"(function(){try{var d=localStorage.getItem('darkmode');if(d==='true'||(d===null&&matchMedia('(prefers-color-scheme: dark)').matches))document.documentElement.classList.add('dark');}catch(e){}})();"}
                </script>
            </head>
            <body class="bg-background text-foreground antialiased">
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    // Dark/light mode: provides the ThemeMode context (used by the toggle) and reflects it
    // onto <html> so the `.dark` token overrides + `dark:` utilities apply.
    let theme = ThemeMode::init();
    Effect::new(move |_| apply_theme(theme.is_dark()));
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
            <main class="p-6">
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
        <Suspense fallback=|| {
            view! { <p class="text-sm text-muted-foreground">"Loading…"</p> }
        }>
            {move || {
                let children = children.clone();
                Suspend::new(async move {
                    match account.await {
                        Ok(Some(_)) => children().into_any(),
                        _ => {
                            redirect_to_login();
                            view! {
                                <p class="text-sm text-muted-foreground">
                                    "Redirecting to log in…"
                                </p>
                            }
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
        <nav class="sticky top-0 z-40 border-b border-border bg-background">
            <div class="flex h-12 items-center gap-6 px-5">
                <a href="/" class="text-sm font-semibold tracking-[0.2em] text-foreground">
                    "VECTOR"
                </a>
                <a
                    href="/maps"
                    class="flex items-center gap-1.5 text-sm text-muted-foreground transition-colors hover:text-foreground"
                >
                    <Map class="size-4" />
                    "Maps"
                </a>

                <span class="ml-auto flex items-center gap-3">
                    <Transition fallback=|| ()>
                        {move || Suspend::new(async move { status_badge(status.await) })}
                    </Transition>
                    <ThemeToggle />
                    <Suspense fallback=|| ()>
                        {move || Suspend::new(async move {
                            let (character, characters) = account.await;
                            match character {
                                Some(character) => account_menu(character, characters),
                                None => login_button(),
                            }
                        })}
                    </Suspense>
                </span>
            </div>
        </nav>
    }
}

/// Minimal menu-item styling: slim, square, monochrome.
const MENU_ITEM: &str = "flex w-full items-center gap-2 whitespace-nowrap px-2 py-1.5 text-left \
                         text-sm text-muted-foreground transition-colors hover:bg-accent \
                         hover:text-foreground";

/// The avatar + account dropdown: switch character, add a character, remove the active one
/// (when more than one), and log out.
fn account_menu(active: CharacterSummary, characters: Vec<CharacterRef>) -> AnyView {
    let active_id = active.character_id;
    let active_name = active.name;
    let can_remove = characters.len() > 1;

    view! {
        <DropdownMenu align=DropdownMenuAlign::End>
            <DropdownMenuTrigger as_child=true>
                <button
                    type="button"
                    aria-label="Account"
                    class="block size-7 overflow-hidden border border-border transition-colors hover:border-foreground/50"
                >
                    <CharacterImage id=active_id class="size-7 object-cover" />
                </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
                {characters
                    .into_iter()
                    .map(|c| {
                        let id = c.character_id;
                        let active_class = if c.is_active { " text-foreground" } else { "" };
                        view! {
                            <button
                                type="button"
                                class=format!("{MENU_ITEM}{active_class}")
                                on:click=move |_| {
                                    spawn_local(async move {
                                        if switch_character(id).await.is_ok() {
                                            reload_page();
                                        }
                                    });
                                }
                            >
                                <CharacterImage id=id class="size-5 border border-border" />
                                <span class="truncate">{c.name}</span>
                            </button>
                        }
                    })
                    .collect_view()}

                <div class="my-1 h-px bg-border"></div>

                <a href="/auth/login?link=true" rel="external" class=MENU_ITEM>
                    <Plus class="size-4" />
                    "Add character"
                </a>

                {can_remove
                    .then(|| {
                        view! {
                            <button
                                type="button"
                                class=format!("{MENU_ITEM} hover:text-destructive")
                                on:click=move |_| {
                                    spawn_local(async move {
                                        if remove_character(active_id).await.is_ok() {
                                            reload_page();
                                        }
                                    });
                                }
                            >
                                <Trash2 class="size-4" />
                                {format!("Remove {active_name}")}
                            </button>
                        }
                    })}

                <div class="my-1 h-px bg-border"></div>

                <a href="/auth/logout" rel="external" class=MENU_ITEM>
                    <LogOut class="size-4" />
                    "Log out"
                </a>
            </DropdownMenuContent>
        </DropdownMenu>
    }
    .into_any()
}

fn login_button() -> AnyView {
    view! {
        <a
            href="/auth/login"
            rel="external"
            class="border border-border px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
            "Log in"
        </a>
    }
    .into_any()
}

/// The active character's live status: a small online dot, ship icon, and current system.
/// Empty when not signed in or not yet tracked.
fn status_badge(status: Option<CharacterStatus>) -> AnyView {
    let Some(s) = status else {
        return ().into_any();
    };
    let dot = if s.online {
        "bg-emerald-500"
    } else {
        "bg-muted-foreground/40"
    };
    let system = s.solar_system.unwrap_or_else(|| "—".into());
    view! {
        <span class="hidden items-center gap-2 text-xs text-muted-foreground md:flex">
            <span class=format!("size-1.5 rounded-full {dot}")></span>
            {s.ship_type_id.map(|id| view! { <TypeImage id=id class="size-4" /> })}
            <span class="tracking-wide">{system}</span>
        </span>
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

/// Reflect the theme onto `<html>` so the `.dark` token overrides + `dark:` utilities apply.
#[cfg(feature = "hydrate")]
fn apply_theme(dark: bool) {
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let classes = root.class_list();
        let _ = if dark {
            classes.add_1("dark")
        } else {
            classes.remove_1("dark")
        };
    }
}

#[cfg(not(feature = "hydrate"))]
fn apply_theme(_dark: bool) {}

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
