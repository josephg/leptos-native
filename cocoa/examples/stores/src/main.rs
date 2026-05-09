//! macOS port of `stores` — demonstrates `reactive_stores`:
//! `Store`, `Field`, and sub-field reactivity.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    use reactive_stores::{Patch, Store};

    #[derive(Debug, Store, Patch)]
    pub struct User {
        name: String,
    }

    impl User {
        fn new(name: &str) -> Self {
            Self { name: name.into() }
        }
    }

    #[component]
    pub fn App() -> impl IntoView {
        let store = Store::new(User::new("Alice"));
        let name_field = store.name();

        view! {
            <vstack padding=16.0 gap=8.0>
                <label>{move || format!("Hello, {}", name_field.get())}</label>

                <text_field
                    value=move || name_field.get()
                    on:change=move |s: String| {
                        store.patch(User { name: s });
                    }
                />

                // Direct store field access via label
                <label>{move || format!("From field: {}", store.name().get())}</label>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Stores", (340.0, 200.0), || {
            view! { <App /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
