use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

#[component]
pub fn ProfilePage() -> impl IntoView {
    let query = use_query_map();
    let param = move |key: &str| query.read().get(key);

    view! {
        <h1>"Profile"</h1>
        {move || match param("name") {
            None => view! { <p>"Not logged in. " <a href="/login">"Log in"</a></p> }.into_any(),
            Some(name) => view! {
                <p>"Login succeeded — logged in as " <strong>{name}</strong> "."</p>
                <h2>"Affiliations"</h2>
                <ul>
                    {move || param("corporation").map(|c| view! { <li>"Corporation: " {c}</li> })}
                    {move || param("alliance").map(|a| view! { <li>"Alliance: " {a}</li> })}
                    {move || param("faction").map(|f| view! { <li>"Faction: " {f}</li> })}
                </ul>
                <h2>"Granted scopes"</h2>
                {move || match param("scopes") {
                    None => view! { <p>"None."</p> }.into_any(),
                    Some(scopes) => {
                        let items: Vec<String> = scopes.split(' ').map(str::to_string).collect();
                        view! {
                            <ul>
                                {items.into_iter().map(|s| view! { <li>{s}</li> }).collect_view()}
                            </ul>
                        }
                        .into_any()
                    }
                }}
            }
            .into_any(),
        }}
    }
}
