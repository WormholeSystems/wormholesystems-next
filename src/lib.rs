#![allow(dead_code)]

pub mod app;

#[cfg(feature = "ssr")]
pub mod auth;
#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod esi;
#[cfg(feature = "ssr")]
pub mod maps;
#[cfg(feature = "ssr")]
pub mod sde;
#[cfg(feature = "ssr")]
pub mod seed;
#[cfg(feature = "ssr")]
pub mod util;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(app::App);
}
