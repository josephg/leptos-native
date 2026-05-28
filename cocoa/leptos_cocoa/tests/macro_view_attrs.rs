//! Exercises view!{} attribute syntaxes: `bind:`, `node_ref=`, layout
//! attrs (`.padding`, `.flex_grow`, etc.).

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

use leptos_cocoa::prelude::*;
use leptos_native::node_ref::NodeRef;

#[component]
fn LayoutAttrs() -> impl IntoView {
    view! {
        <vstack padding=16.0 gap=8.0>
            <hstack flex_grow=1.0>
                <label>"a"</label>
            </hstack>
        </vstack>
    }
}

#[component]
fn BindValue() -> impl IntoView {
    let text = RwSignal::new(String::new());
    view! { <text_field bind:value=text /> }
}

#[component]
fn BindChecked() -> impl IntoView {
    let on = RwSignal::new(false);
    view! { <checkbox bind:checked=on /> }
}

#[component]
fn NodeRefAttr() -> impl IntoView {
    let r: NodeRef<_> = NodeRef::new();
    view! { <text_field node_ref=r /> }
}

#[test]
fn attrs_smoke_compiles() {
    let _ = LayoutAttrsProps::builder().build();
    let _ = BindValueProps::builder().build();
    let _ = BindCheckedProps::builder().build();
    let _ = NodeRefAttrProps::builder().build();
}
