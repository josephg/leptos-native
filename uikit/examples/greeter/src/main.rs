//! Greeter — `bind:value` two-way binding on `<text_field>`.
//! Type into the field; the label below echoes whatever you type.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn Greeter() -> impl IntoView {
        let name = RwSignal::new(String::new());

        view! {
            <vstack padding=16.0 gap=8.0>
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
            </vstack>
        }
    }

    pub fn main() {
        leptos_platform::mount_ios::run(|| view! { <Greeter /> });
    }

}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
