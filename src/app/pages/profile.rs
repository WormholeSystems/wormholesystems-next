use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

#[component]
pub fn ProfilePage() -> impl IntoView {
    let query = use_query_map();
    let param = move |key: &str| query.read().get(key);

    view! {
        <div class="max-w-2xl">
            <h1 class="text-lg font-semibold tracking-tight">"Profile"</h1>
            {move || match param("name") {
                None => view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        "Not logged in. "
                        <a href="/login" class="text-foreground underline">
                            "Log in"
                        </a> "."
                    </p>
                }
                .into_any(),
                Some(name) => view! {
                    <p class="mt-4 text-sm text-muted-foreground">
                        "Logged in as "
                        <span class="font-medium text-foreground">{name}</span> "."
                    </p>

                    <h2 class="mt-6 text-xs font-medium uppercase tracking-wider text-muted-foreground">
                        "Affiliations"
                    </h2>
                    <ul class="mt-2 space-y-0.5 text-sm text-foreground">
                        {move || {
                            param("corporation").map(|c| view! { <li>"Corporation: " {c}</li> })
                        }}
                        {move || param("alliance").map(|a| view! { <li>"Alliance: " {a}</li> })}
                        {move || param("faction").map(|f| view! { <li>"Faction: " {f}</li> })}
                    </ul>

                    <h2 class="mt-6 text-xs font-medium uppercase tracking-wider text-muted-foreground">
                        "Granted scopes"
                    </h2>
                    {move || match param("scopes") {
                        None => view! {
                            <p class="mt-2 text-sm text-muted-foreground">"None."</p>
                        }
                        .into_any(),
                        Some(scopes) => {
                            let items: Vec<String> = scopes
                                .split(' ')
                                .map(str::to_string)
                                .collect();
                            view! {
                                <ul class="mt-2 space-y-0.5 font-mono text-xs text-foreground">
                                    {items
                                        .into_iter()
                                        .map(|s| view! { <li>{s}</li> })
                                        .collect_view()}
                                </ul>
                            }
                            .into_any()
                        }
                    }}
                }
                .into_any(),
            }}
        </div>
    }
}
