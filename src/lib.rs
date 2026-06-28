#![allow(dead_code)]

// Vendored Rust/UI component library (added via the `ui` CLI). Its components re-export
// items that aren't all used yet; don't lint the generated tree.
#[allow(unused_imports)]
mod components;

pub mod app;

#[cfg(feature = "ssr")]
pub mod auth;
#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod esi;
// Compiled for both targets: the data types are shared with the client (server functions);
// the DB actions, sqlx glue, and event hub inside are gated `ssr` (see maps/mod.rs).
pub mod maps;
#[cfg(feature = "ssr")]
pub mod sde;
#[cfg(feature = "ssr")]
pub mod seed;
#[cfg(feature = "ssr")]
pub mod session;
#[cfg(feature = "ssr")]
pub mod sovereignty;
#[cfg(feature = "ssr")]
pub mod tracking;
#[cfg(feature = "ssr")]
pub mod user_channel;
#[cfg(feature = "ssr")]
pub mod util;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    // Install `window.ScrollLock` (used by overlay components) before the app mounts.
    components::hooks::use_scroll_lock::init();
    leptos::mount::hydrate_body(app::App);
}
