//! Greeter — `bind:value` two-way binding on `<text_field>`.
//! Type into the field; the label below echoes whatever you type.

use leptos::prelude::*;

#[component]
fn Greeter() -> impl IntoView {
    let name = RwSignal::new(String::new());

    view! {
        <vstack padding=16.0 gap=8.0>
            <label>{"Your name:"}</label>
            <text_field bind:value=name placeholder="Type here..." />
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
    leptos::mount_ios::run(|| view! { <Greeter /> });
}
