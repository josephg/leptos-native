//! Counter — the canonical first example.
//!
//! Demonstrates: signals, `on:click`, dynamic text in a label.
//!
//! Earliest stage that can run this: **Stage 5** (Cocoa-flavoured
//! element builders + view! macro). Before that, the closest working
//! version is `cocoa_dom/examples/hello_window.rs`, which builds the
//! same tree manually.
//!
//! Status: aspirational — won't compile until Stage 5 lands.

use leptos::prelude::*;

#[component]
fn Counter(initial: i32) -> impl IntoView {
    let count = RwSignal::new(initial);

    view! {
        <stack_view orientation="vertical" spacing=8.0>
            <label>{move || format!("Count: {}", count.get())}</label>
            <stack_view orientation="horizontal" spacing=4.0>
                <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
                <button on:click=move |_| count.set(0)>"Reset"</button>
                <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
            </stack_view>
        </stack_view>
    }
}

fn main() {
    leptos::mount::mount_to_window(|| view! { <Counter initial=0 /> });
}
