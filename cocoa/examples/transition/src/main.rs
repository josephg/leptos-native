//! `<Transition>` + `<ErrorBoundary>` + `Suspend` + `LocalResource`
//! end-to-end. Type a count and the app fetches that many cat
//! facts; while reloading, Transition keeps the previous batch
//! visible (vs Suspense, which would flash to the fallback).
//!
//! Counts ≤ 0 trigger an error caught by ErrorBoundary.

use leptos_native::prelude::*;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
struct Fact {
    fact: String,
}

#[derive(Debug, Clone, Error)]
enum FetchError {
    #[error("count must be > 0")]
    NonPositive,
    #[error("network: {0}")]
    Network(String),
}

async fn fetch_facts(count: usize) -> Result<Vec<String>, FetchError> {
    if count == 0 {
        return Err(FetchError::NonPositive);
    }
    // Bridge: HTTP runs on a tokio worker thread, result flows back
    // via oneshot to the main-thread spawner.
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let res = async {
                let resp = reqwest::get("https://catfact.ninja/fact")
                    .await
                    .map_err(|e| FetchError::Network(e.to_string()))?;
                let f: Fact = resp
                    .json()
                    .await
                    .map_err(|e| FetchError::Network(e.to_string()))?;
                Ok::<_, FetchError>(f.fact)
            }
            .await;
            match res {
                Ok(s) => out.push(s),
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
        let _ = tx.send(Ok(out));
    });
    rx.await
        .map_err(|_| FetchError::Network("cancelled".into()))?
}

#[component]
fn App() -> impl IntoView {
    let count_text = RwSignal::new(String::from("2"));

    // count is a derived signal — parse the text. A parse failure
    // resolves to 0, which fetch_facts treats as an error.
    let count = move || count_text.get().parse::<usize>().unwrap_or(0);

    let facts = LocalResource::new(move || fetch_facts(count()));

    view! {
        <vstack padding=20.0 gap=12.0>
            <label>"How many cat facts?"</label>
            <text_field bind:value=count_text />

            <Transition fallback=|| view! {
                <label>"Loading…"</label>
            }>
                <ErrorBoundary fallback=|errors| {
                    let errors = errors.clone();
                    view! {
                        <vstack gap=4.0>
                            <label>"Error:"</label>
                            <label>{move || {
                                errors.read()
                                    .iter()
                                    .map(|(_, e)| e.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }}</label>
                        </vstack>
                    }
                }>
                    <stack>
                        {move || Suspend::new(async move {
                            facts.await.map(|fs| fs.join("\n\n"))
                        })}
                    </stack>
                </ErrorBoundary>
            </Transition>
        </vstack>
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("Transition + ErrorBoundary", (520.0, 480.0), || {
        view! { <App /> }
    }).run();
}
