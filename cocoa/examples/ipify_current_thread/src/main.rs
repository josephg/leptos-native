//! ipify_current_thread — same as `ipify`, but with tokio's
//! single-threaded scheduler.
//!
//! `Builder::new_current_thread()` builds a runtime that runs all
//! tasks on one thread. We can't tick it from the main thread
//! (AppKit owns that), so we park it on a dedicated side thread:
//!
//! ```text
//!     main thread:    NSApp.run() + reactive graph + UI
//!     side thread:    rt.block_on(future::pending::<()>())
//!                     — drives tokio's reactor & scheduler
//! ```
//!
//! With `Handle::enter()` set on main, `tokio::spawn(...)` from
//! main hands the task off to the side thread, where it runs.
//! From the call site this looks identical to multi-thread tokio
//! — same `tokio::spawn(io).await` pattern from `ipify`.
//!
//! When you'd reach for this:
//! - Smaller resource footprint (one I/O thread instead of N).
//! - Deterministic task ordering (FIFO on a single worker).
//! - Easier reasoning about concurrency for tests.

use leptos::prelude::*;
use serde::Deserialize;
use std::future;

#[derive(Debug, Clone, Deserialize)]
struct IpResponse {
    ip: String,
}

async fn fetch_ip() -> Result<String, String> {
    let handle = tokio::spawn(async {
        let resp = reqwest::get("https://api.ipify.org?format=json")
            .await
            .map_err(|e| e.to_string())?;
        let body: IpResponse =
            resp.json().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(body.ip)
    });
    handle.await.map_err(|e| e.to_string())?
}

#[component]
fn App() -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let ip = AsyncDerived::new(move || {
        let _ = refresh.get();
        async { fetch_ip().await }
    });

    view! {
        <vstack padding=20.0 gap=12.0>
            <label bold=true>"Public IP (current_thread tokio)"</label>
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
    // Build a current_thread runtime with all I/O drivers enabled
    // (so reqwest's TCP/TLS works). Then park it on a side thread
    // — `rt.block_on(future::pending())` claims the side thread
    // forever to drive the runtime's reactor.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio current_thread runtime");
    let handle = rt.handle().clone();
    std::thread::Builder::new()
        .name("tokio-current-thread".into())
        .spawn(move || rt.block_on(future::pending::<()>()))
        .expect("spawn tokio driver thread");

    // `tokio::spawn` resolves the runtime via thread-local; this
    // guard keeps it set on main for the program's lifetime.
    let _guard = handle.enter();

    mount_to_window(
        "ipify (current_thread)",
        (340.0, 160.0),
        || view! { <App /> },
    );
}
