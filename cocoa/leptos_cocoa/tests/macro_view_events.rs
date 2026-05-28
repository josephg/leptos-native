//! `on:event=` smoke test for cocoa events.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

use leptos_cocoa::prelude::*;

#[component]
fn ClickComp() -> impl IntoView {
    view! { <button on:click=move |_| {}>"click"</button> }
}

#[component]
fn InputAndCommit() -> impl IntoView {
    view! {
        <vstack>
            <text_field on:input=move |_s: String| {} />
            <text_field on:commit=move |_s: String| {} />
        </vstack>
    }
}

#[test]
fn events_smoke_compiles() {
    let _ = ClickCompProps::builder().build();
    let _ = InputAndCommitProps::builder().build();
}
