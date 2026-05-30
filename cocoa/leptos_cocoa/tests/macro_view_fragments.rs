//! Multi-root fragments + nested components.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

use leptos_cocoa::prelude::*;

#[component]
fn Inner() -> impl IntoView {
    view! { <label>"inner"</label> }
}

#[component]
fn MultiRoot() -> impl IntoView {
    view! {
        <label>"a"</label>
        <label>"b"</label>
        <label>"c"</label>
    }
}

#[component]
fn Nested() -> impl IntoView {
    view! { <vstack><Inner/></vstack> }
}

#[test]
fn fragments_smoke_compiles() {
    let _ = InnerProps::builder().build();
    let _ = MultiRootProps::builder().build();
    let _ = NestedProps::builder().build();
}
