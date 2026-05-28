//! Counter — using `leptos_platform::prelude::*`, the `view!{}` macro, and
//! `#[component]` against the Linux/GTK backend.

extern crate leptos_gtk as leptos_platform;

mod app {
    use leptos_platform::prelude::*;

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
        mount_to_window(
            "org.leptos.counter_gtk",
            "Counter — view! + #[component]",
            (320, 200),
            || view! { <Counter initial=0 /> },
        )
        .run();
    }
}

fn main() { app::main() }
