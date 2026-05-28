//! Test the `<Component on:click=…>` pattern that Tier 2.F enables.
//! `InnerButton` returns a single `<button>`; the parent attaches an
//! `on:click` handler to the component itself, and it propagates to
//! the button.

extern crate leptos_cocoa as leptos_platform;

#[cfg(target_os = "macos")]
mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn InnerButton() -> impl IntoView {
        view! { <button>"click me"</button> }
    }

    #[component]
    pub fn App() -> impl IntoView {
        let count = RwSignal::new(0);
        view! {
            <vstack padding=16.0 gap=8.0>
                <label>{move || format!("Count: {}", count.get())}</label>
                <InnerButton on:click=move |_| count.update(|n| *n += 1) />
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Component on:click test", (320.0, 200.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
