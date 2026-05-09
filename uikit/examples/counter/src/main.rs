//! Counter — iOS UIKit port using `view!{}` and `#[component]`.

use leptos::prelude::*;

#[component]
fn Counter(initial: i32) -> impl IntoView {
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

fn main() {
    leptos::mount_ios::run(|| view! { <Counter initial=0 /> });
}

