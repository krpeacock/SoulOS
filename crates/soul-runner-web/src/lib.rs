//! Web wasm entry point for SoulOS.
//!
//! Trunk-driven build: `trunk serve` rebuilds and reloads the browser on
//! every source change. The Rust side currently does the bare minimum —
//! it locates the `#app` slot in `index.html` and writes the product name
//! into it — but the entry point is wired so future work can hand off to
//! `soul_core::run` with a wasm `Platform` impl, the way the desktop
//! binary already does.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let document = web_sys::window()
        .and_then(|w| w.document())
        .expect("document");
    let app = document
        .get_element_by_id("app")
        .expect("missing #app element in index.html");
    app.set_text_content(Some("SoulOS"));
}
