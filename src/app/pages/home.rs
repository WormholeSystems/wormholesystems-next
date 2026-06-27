use leptos::prelude::*;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <h1>"Vector"</h1>
        <p>"Wormhole mapping for EVE Online — under construction."</p>
        <p>
            <a href="/login">"Log in"</a>
        </p>
    }
}
