//! Multi-window demo: two independent NSWindows, each with its own
//! state and Taffy tree, sharing nothing.
//!
//! Run with:
//!     cargo run --manifest-path cocoa/examples/two_windows/Cargo.toml
//!
//! Validates the multi-window architecture: `leptos::mount::run`
//! accepts a tuple of `Window<…>` builders; each builder opens its own
//! NSWindow on `Render::build`. Quitting either window or Cmd-Q quits
//! the whole app.
//!
//! For a single-window app, use `mount_to_window` instead — it's a
//! one-liner that wraps your view in a `window()` builder for you.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    use leptos::tachys::html::element::window;

    pub fn main() {
        // Two independent counters — each window has its own state, with
        // no shared signals. Either window's buttons only affect its own
        // count.
        let main_count = RwSignal::new(0_i32);
        let inspector_count = RwSignal::new(100_i32);

        run(move || {
            (
                // Window 1 — main counter
                window().title("Main").size((360.0, 200.0)).child(view! {
                    <vstack padding=16.0 gap=12.0>
                        <label>{move || format!("Main count: {}", main_count.get())}</label>
                        <hstack gap=8.0>
                            <button on:click=move |_| main_count.update(|n| *n -= 1)>"-1"</button>
                            <button on:click=move |_| main_count.update(|n| *n += 1)>"+1"</button>
                        </hstack>
                    </vstack>
                }),
                // Window 2 — independent inspector
                window().title("Inspector").size((280.0, 160.0)).child(view! {
                    <vstack padding=12.0 gap=8.0>
                        <label>{move || format!("Inspector: {}", inspector_count.get())}</label>
                        <button on:click=move |_| inspector_count.set(100)>"Reset to 100"</button>
                        <button on:click=move |_| inspector_count.update(|n| *n += 10)>"Add 10"</button>
                    </vstack>
                }),
            )
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
