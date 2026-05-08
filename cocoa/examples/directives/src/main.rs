//! Demonstrates `use:directive` on macOS. Directives are functions
//! that run at `Render::build` time, receiving the constructed
//! element plus an optional parameter.

use cocoa_dom::Element;
use leptos::prelude::*;

fn log_build(_el: Element) {
    eprintln!("[directive] log_build: element built");
}

fn with_param(_el: Element, msg: &'static str) {
    eprintln!("[directive] with_param: {msg}");
}

#[component]
fn App() -> impl IntoView {
    view! {
        <vstack padding=16.0 gap=8.0>
            <button use:log_build use:with_param="button built">
                "Click me"
            </button>
            <text_field use:log_build use:with_param="field built" />
        </vstack>
    }
}

fn main() {
    mount_to_window("Directives", (320.0, 160.0), || {
        view! { <App /> }
    });
}
