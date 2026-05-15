//! ipify — fetch and display the user's public IP address.
//!
//! The "hello world" of async integration. Demonstrates **pattern
//! 1** from the async docs: `tokio::spawn(io).await` from inside a
//! main-thread `AsyncDerived`.
//!
//! Threading model:
//!
//! - Main thread: AppKit run loop, reactive graph, `AsyncDerived`'s
//!   closure body, every signal mutation.
//! - Tokio worker pool: the actual `reqwest::get(…)` future. Owns
//!   its mio reactor, kqueues, TCP socket, TLS state.
//!
//! The bridge is `tokio::spawn(fut).await`: the `JoinHandle`
//! returned by `tokio::spawn` is itself a `Send` future that can
//! be polled from any thread, so our main-thread executor picks it
//! up when tokio completes the task. No oneshot needed.

use leptos::prelude::*;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct IpResponse {
    ip: String,
}

async fn fetch_ip() -> Result<String, String> {
    // Off-main: HTTP on a tokio worker.
    let handle = tokio::spawn(async {
        let resp = reqwest::get("https://api.ipify.org?format=json")
            .await
            .map_err(|e| e.to_string())?;
        let body: IpResponse =
            resp.json().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(body.ip)
    });
    // Back on main: await the JoinHandle. JoinHandle: Future, and
    // polling it doesn't require a tokio context — only the inner
    // task does. `??` collapses JoinError + reqwest error.
    handle.await.map_err(|e| e.to_string())?
}

#[component]
fn App() -> impl IntoView {
    // Bump this signal to re-trigger the AsyncDerived (it's read
    // inside the closure, so the dependency is automatic).
    let refresh = RwSignal::new(0u32);
    let ip = AsyncDerived::new(move || {
        let _ = refresh.get();
        async { fetch_ip().await }
    });

    view! {
        <vstack padding=20.0 gap=12.0>
            <label bold=true>"Your public IP address"</label>
            <label>{move || match ip.get() {
                Some(Ok(ip)) => ip,
                Some(Err(e)) => format!("Error: {e}"),
                None         => "Fetching…".to_string(),
            }}</label>
            <button on:click=move |_| refresh.update(|n| *n += 1)>
                "Refresh"
            </button>
        </vstack>
    }
}

fn main() {
    // User owns the runtime. Multi-thread = workers drive the
    // reactor on their own; we just `tokio::spawn` from main.
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("ipify", (320.0, 140.0), || view! { <App /> });
}
