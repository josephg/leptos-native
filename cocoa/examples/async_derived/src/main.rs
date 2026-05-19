//! `AsyncDerived` — derive a value from a future. The closure
//! runs whenever a signal it reads changes; subscribers see
//! `None` until the future resolves and `Some(value)` after.
//!
//! Type a number into the field. The "result" is the doubled
//! value, computed asynchronously with a 600 ms simulated
//! delay. Editing the field cancels the in-flight derivation
//! and starts a new one.
//!
//! The tokio runtime is started before `mount_to_window` so
//! `tokio::time::sleep` has a reactor to use.

use leptos_native::prelude::*;
use std::time::Duration;

async fn slow_double(n: i32) -> i32 {
    tokio::time::sleep(Duration::from_millis(600)).await;
    n * 2
}

#[component]
fn App() -> impl IntoView {
    let input = RwSignal::new(String::from("21"));
    let n = move || input.get().parse::<i32>().unwrap_or(0);

    let doubled = AsyncDerived::new(move || async move { slow_double(n()).await });

    view! {
        <vstack padding=20.0 gap=12.0>
            <label bold=true>"AsyncDerived — type a number"</label>
            <text_field bind:value=input />

            <label>{move || match doubled.get() {
                Some(v) => format!("doubled = {v}"),
                None    => "computing…".to_string(),
            }}</label>

            <label text_color=Color::GRAY>
                "The result lags input by ~600ms — that's the await delay."
            </label>
        </vstack>
    }
}

fn main() {
    // AsyncDerived's future runs on the leptos spawner, but
    // `tokio::time::sleep` needs a tokio reactor. Build a
    // single-thread runtime and enter it for the lifetime of
    // the app.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("AsyncDerived", (380.0, 200.0), || view! { <App /> }).run();
}
