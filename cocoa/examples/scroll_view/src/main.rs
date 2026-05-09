//! `<scroll_view>` smoke test — a vstack of N rows wrapped in a
//! scroll view. Add/Remove buttons resize the list to test that
//! the documentView grows past the viewport (and shrinks back).

use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    let count = RwSignal::new(30_usize);

    view! {
        // flex_grow=1 on the outer vstack tells the window's content
        // root "give me all your height" — without this, the outer
        // vstack sizes to content and the scroll_view inside has no
        // bounded viewport to clip against, so it grows to its
        // content's natural height instead of scrolling.
        <vstack padding=12.0 gap=8.0 flex_grow=1.0>
            <hstack gap=8.0>
                <button on:click=move |_| count.update(|n| *n += 5)>"Add 5"</button>
                <button on:click=move |_| count.update(|n| *n = n.saturating_sub(5))>"Remove 5"</button>
                <label>{move || format!("{} rows", count.get())}</label>
            </hstack>

            <scroll_view flex_grow=1.0>
                <vstack gap=2.0>
                    <For
                        each=move || {
                            let n = count.get();
                            (0..n).collect::<Vec<usize>>()
                        }
                        key=|i| *i
                        children=move |i| view! {
                            <label>{move || format!("Row {i}")}</label>
                        }
                    />
                </vstack>
            </scroll_view>
        </vstack>
    }
}

fn main() {
    mount_to_window("Scroll view", (320.0, 360.0), || {
        view! { <App /> }
    });
}
