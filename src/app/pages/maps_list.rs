//! The signed-in user's maps: list, create, open, delete. Scoped to the active character
//! via the session (the server functions resolve the actor themselves).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::api::{MapEntry, create_map, delete_map, my_maps};

#[component]
pub fn MapsPage() -> impl IntoView {
    let reload = RwSignal::new(0u32);
    let maps = Resource::new(move || reload.get(), |_| async move { my_maps().await });
    let new_name = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());

    let create = move |_| {
        let name = new_name.get_untracked().trim().to_string();
        if name.is_empty() {
            return;
        }
        spawn_local(async move {
            match create_map(name).await {
                Ok(_) => {
                    new_name.set(String::new());
                    error.set(String::new());
                    reload.update(|n| *n += 1);
                }
                Err(err) => error.set(err.to_string()),
            }
        });
    };

    let remove = move |id: i64| {
        spawn_local(async move {
            match delete_map(id).await {
                Ok(()) => reload.update(|n| *n += 1),
                Err(err) => error.set(err.to_string()),
            }
        });
    };

    view! {
        <div class="max-w-2xl">
            <h1 class="text-lg font-semibold tracking-tight">"Your maps"</h1>
            <p class="mt-1 h-4 text-sm text-destructive">{move || error.get()}</p>

            <div class="mt-4 flex gap-2">
                <input
                    class="flex-1 border border-border bg-background px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus-visible:border-foreground/40"
                    placeholder="New map name"
                    prop:value=move || new_name.get()
                    on:input=move |ev| new_name.set(event_target_value(&ev))
                    on:keydown=move |ev| if ev.key() == "Enter" { create(()) }
                />
                <button
                    class="border border-border bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                    on:click=move |_| create(())
                >
                    "Create"
                </button>
            </div>

            <Suspense fallback=|| {
                view! { <p class="mt-6 text-sm text-muted-foreground">"Loading…"</p> }
            }>
                {move || Suspend::new(async move {
                    match maps.await {
                        Err(_) => view! {
                            <p class="mt-6 text-sm text-muted-foreground">
                                "Please "
                                <a href="/auth/login" rel="external" class="text-foreground underline">
                                    "log in"
                                </a> "."
                            </p>
                        }
                        .into_any(),
                        Ok(list) if list.is_empty() => {
                            view! {
                                <p class="mt-6 text-sm text-muted-foreground">"No maps yet."</p>
                            }
                            .into_any()
                        }
                        Ok(list) => {
                            view! {
                                <ul class="mt-6 divide-y divide-border border-y border-border">
                                    {list.into_iter().map(|m| map_row(m, remove)).collect_view()}
                                </ul>
                            }
                            .into_any()
                        }
                    }
                })}
            </Suspense>
        </div>
    }
}

fn map_row(m: MapEntry, remove: impl Fn(i64) + Copy + 'static) -> impl IntoView {
    let id = m.id;
    view! {
        <li class="group flex items-center justify-between py-2.5">
            <a
                href=format!("/maps/{id}")
                class="text-sm text-foreground transition-colors hover:text-muted-foreground"
            >
                {m.name}
            </a>
            <span class="flex items-center gap-4">
                <span class="text-[11px] uppercase tracking-wider text-muted-foreground">
                    {m.role}
                </span>
                <button
                    class="text-xs text-muted-foreground opacity-0 transition hover:text-destructive group-hover:opacity-100"
                    on:click=move |_| remove(id)
                >
                    "Delete"
                </button>
            </span>
        </li>
    }
}
