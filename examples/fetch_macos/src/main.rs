//! HTTP fetch over `reqwest` from a Leptos macOS app.
//!
//! Demonstrates `Resource` with the `tokio::spawn` + `oneshot`
//! bridge pattern — HTTP runs on a tokio worker thread, result
//! flows back via channel to our main-thread spawner.
//!
//! Multi-threaded tokio runtime keeps IO workers alive while the
//! AppKit run loop blocks the main thread.

use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct Fact {
    fact: String,
}

async fn fetch_cat_fact() -> Result<String, String> {
    // Bridge: run HTTP on a tokio worker thread (full reactor),
    // send result back via oneshot channel. Our spawner polls the
    // receiver on the main thread — no tokio context needed.
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let result = async {
            let resp = reqwest::get("https://catfact.ninja/fact")
                .await.map_err(|e| e.to_string())?;
            let fact: Fact = resp.json().await
                .map_err(|e| e.to_string())?;
            Ok(fact.fact)
        }.await;
        let _ = tx.send(result);
    });
    rx.await.map_err(|_| String::from("cancelled"))?
}

#[component]
fn App() -> impl IntoView {
    let (fetch_count, set_fetch_count) = signal(0_usize);
    let fact = Resource::new(
        move || fetch_count.get(),
        |_| fetch_cat_fact(),
    );

    view! {
        <vstack padding=16.0 gap=8.0>
            <label>{"Cat fact of the moment:"}</label>
            <label>{move || {
                fact.get().map_or_else(
                    || "Loading…".to_string(),
                    |r| match r {
                        Ok(s) => s,
                        Err(e) => format!("Error: {e}"),
                    },
                )
            }}</label>
            <button on:click=move |_| set_fetch_count.update(|n| *n += 1)>
                "Fetch another"
            </button>
        </vstack>
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("Fetch — cat fact", (380.0, 240.0), || {
        view! { <App /> }
    });
}
