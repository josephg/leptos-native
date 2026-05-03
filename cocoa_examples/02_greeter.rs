//! Greeter — minimal demonstration of two-way `bind:`.
//!
//! Demonstrates: `<text_field>` bound to an `RwSignal<String>`, and a
//! reactive `<label>` that recomputes whenever the signal changes.
//!
//! Earliest stage that can run this: **Stage 5+** (after the
//! `bind:` rebuild for Cocoa controls — see implementation_log.md).
//!
//! Status: aspirational — won't compile yet.

use leptos::prelude::*;

#[component]
fn Greeter() -> impl IntoView {
    let name = RwSignal::new(String::new());

    view! {
        <stack_view orientation="vertical" spacing=8.0>
            <label>"Your name:"</label>
            <text_field bind:value=name placeholder="Type here..." />
            <label>{move || {
                let n = name.get();
                if n.is_empty() {
                    "Hello, stranger.".to_string()
                } else {
                    format!("Hello, {n}!")
                }
            }}</label>
        </stack_view>
    }
}

fn main() {
    leptos::mount::mount_to_window(|| view! { <Greeter /> });
}
