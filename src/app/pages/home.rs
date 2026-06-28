use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <h1 class="text-3xl font-bold">"Vector"</h1>
        <p class="mt-2 text-gray-600">"Wormhole mapping for EVE Online."</p>
        <p class="mt-4">
            <a href="/maps" class="text-blue-600 underline">"Your maps"</a>
            " — log in from the top bar if you haven't yet."
        </p>
    }
}
