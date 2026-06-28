use icons::Map;
use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <section class="py-20">
            <p class="text-xs font-medium uppercase tracking-[0.25em] text-muted-foreground">
                "EVE Online"
            </p>
            <h1 class="mt-3 text-4xl font-semibold tracking-tight">"Vector"</h1>
            <p class="mt-3 max-w-prose text-muted-foreground">
                "Real-time wormhole mapping — chains, signatures, and live pilot tracking."
            </p>
            <a
                href="/maps"
                class="mt-8 inline-flex items-center gap-2 border border-border bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
                <Map class="size-4" />
                "Open your maps"
            </a>
        </section>
    }
}
