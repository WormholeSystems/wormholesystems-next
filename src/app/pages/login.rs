use leptos::prelude::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    // `rel="external"` stops the Leptos router from intercepting the click as an
    // in-app route — we want a full navigation to the server route, which 302s to EVE.
    view! {
        <h1>"Log in"</h1>
        <a href="/auth/login" rel="external">
            "Log in with EVE Online"
        </a>
    }
}
