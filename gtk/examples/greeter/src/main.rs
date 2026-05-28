//! Greeter — demonstrates `bind:value` two-way binding on
//! `<text_field>`. Type into the field; the label below echoes
//! whatever you type.

extern crate leptos_gtk as leptos_platform;

mod app {
    use leptos_platform::prelude::*;

    #[component]
    pub fn Greeter() -> impl IntoView {
        let name = RwSignal::new(String::new());

        view! {
            <vstack padding=16.0 gap=8.0>
                <label>"Your name:"</label>
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

    pub fn main() {
        mount_to_window(
            "org.leptos.greeter_gtk",
            "Greeter — bind:value",
            (360, 200),
            || view! { <Greeter /> },
        )
        .run();
    }
}

fn main() { app::main() }
