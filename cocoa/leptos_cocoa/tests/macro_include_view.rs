//! `include_view!` resolves a file relative to the crate root.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

use leptos_cocoa::prelude::*;
use leptos_macro::include_view;

#[component]
fn FromFile() -> impl IntoView {
    include_view!("cocoa/leptos_cocoa/tests/fixtures/included.view")
}

#[test]
fn include_view_compiles() {
    let _ = FromFileProps::builder().build();
}
