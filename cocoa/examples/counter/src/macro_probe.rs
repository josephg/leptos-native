//! Tiny standalone probe: write the smallest possible `view!{}` call
//! and try to build it on macOS, to see exactly what the macro
//! references that we need to provide. NOT compiled into the binary
//! — kept here as a workspace for the view! macro work.

#![allow(dead_code, unused_imports)]

use leptos_platform::prelude::*;

fn _smallest() -> impl IntoView {
    view! { "hello" }
}

fn _just_a_button() -> impl IntoView {
    view! { <button>"Click me"</button> }
}

fn _button_with_handler() -> impl IntoView {
    view! { <button on:click=move |_| ()>"Click"</button> }
}

fn _composed() -> impl IntoView {
    view! {
        <vstack padding=16.0 gap=12.0>
            <label>"Hello"</label>
            <button on:click=move |_| ()>"OK"</button>
        </vstack>
    }
}

#[component]
fn _Greeting(name: String) -> impl IntoView {
    view! { <label>{format!("Hello, {name}")}</label> }
}

fn _uses_component() -> impl IntoView {
    view! {
        <vstack>
            <_Greeting name="world".to_string() />
        </vstack>
    }
}
