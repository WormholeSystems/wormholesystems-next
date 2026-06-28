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
        <h1 class="text-2xl font-bold">"Your maps"</h1>
        <p class="mt-1 text-sm text-red-600">{move || error.get()}</p>

        <div class="mt-4 flex gap-2">
            <input
                class="border rounded px-2 py-1 text-sm flex-1"
                placeholder="New map name"
                prop:value=move || new_name.get()
                on:input=move |ev| new_name.set(event_target_value(&ev))
            />
            <button class="px-3 py-1.5 rounded bg-blue-600 text-white text-sm" on:click=create>
                "Create"
            </button>
        </div>

        <Suspense fallback=|| {
            view! { <p class="mt-4 text-slate-500">"Loading…"</p> }
        }>
            {move || Suspend::new(async move {
                match maps.await {
                    Err(_) => view! {
                        <p class="mt-4">
                            "Please "
                            <a href="/auth/login" rel="external" class="text-blue-600 underline">
                                "log in"
                            </a> "."
                        </p>
                    }
                    .into_any(),
                    Ok(list) if list.is_empty() => {
                        view! { <p class="mt-4 text-slate-500">"No maps yet."</p> }.into_any()
                    }
                    Ok(list) => {
                        view! {
                            <ul class="mt-4 divide-y">
                                {list
                                    .into_iter()
                                    .map(|m| map_row(m, remove))
                                    .collect_view()}
                            </ul>
                        }
                        .into_any()
                    }
                }
            })}
        </Suspense>
    }
}

fn map_row(m: MapEntry, remove: impl Fn(i64) + Copy + 'static) -> impl IntoView {
    let id = m.id;
    view! {
        <li class="flex items-center justify-between py-2">
            <a href=format!("/maps/{id}") class="text-blue-600 hover:underline">
                {m.name}
            </a>
            <span class="flex items-center gap-3">
                <span class="text-xs uppercase text-slate-400">{m.role}</span>
                <button
                    class="text-xs text-red-600 hover:underline"
                    on:click=move |_| remove(id)
                >
                    "delete"
                </button>
            </span>
        </li>
    }
}
