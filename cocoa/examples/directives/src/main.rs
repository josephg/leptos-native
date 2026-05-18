//! Demonstrates `use:directive` on macOS. Directives are functions
//! that run at `Render::build` time, receiving the constructed
//! element plus an optional parameter.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    pub fn log_build(_el: Element) {
        eprintln!("[directive] log_build: element built");
    }

    pub fn with_param(_el: Element, msg: &'static str) {
        eprintln!("[directive] with_param: {msg}");
    }

    #[component]
    pub fn App() -> impl IntoView {
        view! {
            <vstack padding=16.0 gap=8.0>
                <button use:log_build use:with_param="button built">
                    "Click me"
                </button>
                <text_field use:log_build use:with_param="field built" />
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Directives", (320.0, 160.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
