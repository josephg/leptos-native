//! Counter whose value survives app restarts via NSUserDefaults.
//!
//! Demonstrates `local_storage()` — the macOS analog of web's
//! `window.localStorage`. Same `Result<Option<Storage>, _>` shape
//! so example code from the upstream Leptos `todomvc` example
//! ports across with one substitution (`window().local_storage()`
//! → `local_storage()`).

use leptos::prelude::*;

const KEY: &str = "leptos_macos.persistent_counter.value";

fn load_initial() -> i32 {
    local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(KEY).ok().flatten())
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0)
}

fn save(value: i32) {
    if let Ok(Some(storage)) = local_storage() {
        let _ = storage.set_item(KEY, &value.to_string());
    }
}

#[component]
fn PersistentCounter() -> impl IntoView {
    let count = RwSignal::new(load_initial());

    // Persist on every change.
    Effect::new(move |_| {
        save(count.get());
    });

    view! {
        <vstack padding=16.0 gap=12.0>
            <label>{"This counter persists across app launches."}</label>
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
    mount_to_window("Persistent Counter", (340.0, 200.0), || {
        view! { <PersistentCounter /> }
    });
}
