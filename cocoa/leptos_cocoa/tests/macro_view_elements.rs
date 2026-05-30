//! Smoke-tests the `view!{}` macro produces compilable code for the
//! element builders re-exported by the cocoa port. Wrapped in
//! `#[component]` so the resulting view types pick up the `Send`
//! bound through `untrack_with_diagnostics`.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

use leptos_cocoa::prelude::*;

#[component]
fn LeafBuilders() -> impl IntoView {
    view! {
        <vstack>
            <button>"click"</button>
            <label>"text"</label>
            <text_field />
            <hstack>
                <label>"x"</label>
            </hstack>
        </vstack>
    }
}

#[component]
fn Nested() -> impl IntoView {
    view! {
        <vstack>
            <hstack>
                <button>"a"</button>
                <button>"b"</button>
            </hstack>
            <vstack>
                <label>"c"</label>
            </vstack>
        </vstack>
    }
}

#[test]
fn elements_smoke_compiles() {
    let _ = LeafBuildersProps::builder().build();
    let _ = NestedProps::builder().build();
}
