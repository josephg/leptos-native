//! Greeter — demonstrates `bind:value` two-way binding on
//! `<text_field>`. Type into the field; the label below echoes
//! whatever you type.

use leptos::prelude::*;

#[component]
fn Greeter() -> impl IntoView {
    let name = RwSignal::new(String::new());

    // Note: wrap purely-static text content in `{}` — without it the
    // view! macro classifies the subtree as "inert" and emits an
    // `InertElement::new("<html>...</html>")` that we don't handle on
    // macOS.
    view! {
        <vstack padding=16.0 gap=8.0>
            <label>{"Your name:"}</label>
            <text_field bind:value=name />
            <label>{move || {
                let n = name.get();
                if n.is_empty() {
                    "Hello, stranger.".to_string()
                } else {
                    format!("Hello, {n}!")
                }
            }}</label>
        </vstack>
    }
}

fn main() {
    mount_to_window("Greeter — bind:value", (360.0, 200.0), || {
        view! { <Greeter /> }
    });
}
