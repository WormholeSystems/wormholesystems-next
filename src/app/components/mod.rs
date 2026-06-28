//! App-specific widgets (distinct from the vendored `crate::components::ui` library).
//!
//! EVE image components wrap CCP's image server (`images.evetech.net`): pass the entity id
//! and a `class` for sizing/shape. They render a plain `<img>`, so they work in SSR + hydrate
//! with no JS.

use leptos::prelude::*;

mod system_search;
pub use system_search::SystemSearchDialog;

const IMAGE_SERVER: &str = "https://images.evetech.net";

/// A character portrait.
#[component]
pub fn CharacterImage(id: i64, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <img
            src=format!("{IMAGE_SERVER}/characters/{id}/portrait?size=64")
            class=class
            alt=""
            loading="lazy"
        />
    }
}

/// A corporation logo.
#[component]
pub fn CorporationImage(id: i64, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <img
            src=format!("{IMAGE_SERVER}/corporations/{id}/logo?size=64")
            class=class
            alt=""
            loading="lazy"
        />
    }
}

/// An alliance logo.
#[component]
pub fn AllianceImage(id: i64, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <img
            src=format!("{IMAGE_SERVER}/alliances/{id}/logo?size=64")
            class=class
            alt=""
            loading="lazy"
        />
    }
}

/// An item/ship type icon.
#[component]
pub fn TypeImage(id: i64, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <img src=format!("{IMAGE_SERVER}/types/{id}/icon?size=64") class=class alt="" loading="lazy" />
    }
}
