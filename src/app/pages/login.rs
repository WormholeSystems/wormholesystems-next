use leptos::prelude::*;

#[component]
pub fn LoginPage() -> impl IntoView {
    // `rel="external"` stops the Leptos router from intercepting the click — we want a full
    // navigation to the server route, which 302s to EVE.
    view! {
        <div class="grid min-h-[60vh] place-items-center">
            <div class="w-full max-w-sm border border-border bg-card p-8 text-center">
                <h1 class="text-xl font-semibold tracking-tight">"Sign in"</h1>
                <p class="mt-2 text-sm text-muted-foreground">
                    "Authenticate with your EVE Online character."
                </p>
                <a
                    href="/auth/login"
                    rel="external"
                    class="mt-6 inline-flex w-full items-center justify-center border border-border bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
                >
                    "Log in with EVE Online"
                </a>
            </div>
        </div>
    }
}
