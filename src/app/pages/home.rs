use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <h1 class="text-3xl font-bold">"Vector"</h1>
        <p class="mt-2 text-gray-600">"Wormhole mapping for EVE Online — under construction."</p>
        <p class="mt-4">
            <a href="/login" class="text-blue-600 underline">"Log in"</a>
        </p>
    }
}
