//! A command-palette style picker for adding a solar system to a map.
//!
//! Opens as a centered modal (driven by the `open` signal), searches the SDE by name
//! server-side as you type, and calls `on_select` with the chosen `solar_system_id`. Fully
//! Leptos-reactive: keyboard navigation (↑/↓/Enter/Esc) and the result list are driven by
//! signals rather than the vendored `Command` component's static-list `<script>`, which
//! doesn't cope with live server-side results.

use icons::Search;
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::app::api::{SystemSearchResult, search_systems};

#[component]
pub fn SystemSearchDialog(
    /// Visibility, owned by the caller. Set `false` to close.
    open: RwSignal<bool>,
    /// Called with the chosen solar system id when a result is picked.
    #[prop(into)]
    on_select: Callback<i64>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<SystemSearchResult>::new());
    let highlight = RwSignal::new(0usize);
    // Monotonic request id: drop responses that arrive out of order while typing.
    let generation = RwSignal::new(0u32);
    let input_ref = NodeRef::<html::Input>::new();

    // Reset and focus the field each time the dialog opens.
    Effect::new(move |_| {
        if open.get() {
            query.set(String::new());
            results.set(Vec::new());
            highlight.set(0);
            if let Some(el) = input_ref.get() {
                let _ = el.focus();
            }
        }
    });

    let run_search = move |text: String| {
        query.set(text.clone());
        highlight.set(0);
        let request = generation.get_untracked() + 1;
        generation.set(request);
        spawn_local(async move {
            let found = search_systems(text).await.unwrap_or_default();
            if generation.get_untracked() == request {
                results.set(found);
            }
        });
    };

    let close = move || open.set(false);

    let choose = move |id: i64| {
        on_select.run(id);
        open.set(false);
    };

    let on_key = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "Escape" => {
            ev.prevent_default();
            close();
        }
        "ArrowDown" => {
            ev.prevent_default();
            let len = results.get_untracked().len();
            if len > 0 {
                highlight.update(|h| *h = (*h + 1).min(len - 1));
            }
        }
        "ArrowUp" => {
            ev.prevent_default();
            highlight.update(|h| *h = h.saturating_sub(1));
        }
        "Enter" => {
            ev.prevent_default();
            if let Some(s) = results.get_untracked().get(highlight.get_untracked()) {
                choose(s.id);
            }
        }
        _ => {}
    };

    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div class="fixed inset-0 z-50 bg-black/60" on:click=move |_| close()></div>
            <div class="fixed left-1/2 top-[12vh] z-50 w-full max-w-lg -translate-x-1/2 border border-border bg-popover text-popover-foreground shadow-2xl">
                <div class="flex items-center gap-2 border-b border-border px-3">
                    <Search class="size-4 shrink-0 text-muted-foreground" />
                    <input
                        node_ref=input_ref
                        class="h-11 w-full bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none"
                        placeholder="Search for a system…"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || query.get()
                        on:input=move |ev| run_search(event_target_value(&ev))
                        on:keydown=on_key
                    />
                </div>
                <div class="max-h-80 overflow-y-auto p-1">
                    {move || {
                        let rows = results.get();
                        if rows.is_empty() {
                            let msg = if query.get().trim().len() < 2 {
                                "Type at least two characters to search."
                            } else {
                                "No systems found."
                            };
                            return view! {
                                <p class="px-3 py-6 text-center text-sm text-muted-foreground">
                                    {msg}
                                </p>
                            }
                                .into_any();
                        }
                        let hl = highlight.get();
                        rows.into_iter()
                            .enumerate()
                            .map(|(i, s)| system_row(s, i, i == hl, choose, highlight))
                            .collect_view()
                            .into_any()
                    }}
                </div>
            </div>
        </Show>
    }
}

fn system_row(
    s: SystemSearchResult,
    index: usize,
    selected: bool,
    choose: impl Fn(i64) + Copy + 'static,
    highlight: RwSignal<usize>,
) -> impl IntoView {
    let id = s.id;
    let (badge, badge_color) = classification(&s);
    let row_bg = if selected { " bg-accent" } else { "" };

    view! {
        <button
            type="button"
            class=format!(
                "flex w-full items-center gap-3 px-3 py-2 text-left text-sm transition-colors hover:bg-accent{row_bg}",
            )
            on:mouseenter=move |_| highlight.set(index)
            on:click=move |_| choose(id)
        >
            <span class=format!("w-12 shrink-0 font-mono text-xs {badge_color}")>{badge}</span>
            <span class="truncate text-foreground">{s.name}</span>
            <span class="ml-auto truncate text-xs text-muted-foreground">{s.region}</span>
        </button>
    }
}

/// A short class/security label and its color: wormhole class for w-space, otherwise the
/// security status (green high-sec, amber low-sec, red null-sec).
fn classification(s: &SystemSearchResult) -> (String, &'static str) {
    match s.wormhole_class_id {
        Some(c @ 1..=6) => (
            format!("C{c}"),
            if c <= 3 {
                "text-amber-400"
            } else {
                "text-red-400"
            },
        ),
        Some(12) => ("Thera".into(), "text-purple-400"),
        Some(13) => ("Frig".into(), "text-amber-400"),
        Some(14..=18) => ("Drifter".into(), "text-red-400"),
        Some(25) => ("Pochven".into(), "text-red-400"),
        _ => {
            let sec = if s.security.abs() < 0.05 {
                0.0
            } else {
                s.security
            };
            let color = if sec >= 0.45 {
                "text-emerald-400"
            } else if sec > 0.0 {
                "text-amber-400"
            } else {
                "text-red-400"
            };
            (format!("{sec:.1}"), color)
        }
    }
}
