//! Counter — using `leptos::prelude::*`, the `view!{}` macro, and
//! `#[component]`.
//!
//! Stage 5 part 3 (slice 2): IntoView + #[component] now work on
//! macOS. Components return `impl IntoView` and can be invoked with
//! `<MyComponent prop=value />` syntax inside `view!{}`.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    #[component]
    pub fn Counter(initial: i32) -> impl IntoView {
        let count = RwSignal::new(initial);

        view! {
            <vstack padding=16.0 gap=12.0>
                <label>{move || format!("Count: {}", count.get())}</label>
                <hstack gap=8.0>
                    <button on:click=move |_| count.update(|n| *n -= 1)>"-1"</button>
                    <button on:click=move |_| count.set(0)>"Reset"</button>
                    <button on:click=move |_| count.update(|n| *n += 1)>"+1"</button>
                </hstack>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Counter — view! + #[component]", (320.0, 200.0), || {
            view! { <Counter initial=0 /> }
        });
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
