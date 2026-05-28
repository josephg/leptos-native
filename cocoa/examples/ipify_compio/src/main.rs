//! ipify_compio — same fetch demo as `ipify`, but with compio
//! instead of tokio as the off-main async runtime.
//!
//! Demonstrates that the framework's bridging story is
//! runtime-agnostic. The cocoa port's main-thread spawner doesn't
//! care which runtime sits on the other side of the channel — all
//! it sees is a `Send` `Future` (here a `futures_channel::oneshot`
//! receiver) that resolves when the off-main work finishes.
//!
//! ## Threading model
//!
//! Compio is a thread-per-core runtime: `compio::Runtime` is
//! `Rc`-based and lives in a thread-local. We can't enter the
//! same runtime on multiple threads, so we use
//! `compio_dispatcher::Dispatcher` — it spawns a pool of worker
//! threads each running its own compio runtime, and gives us a
//! `Send` `dispatch()` method that ferries work onto one of them.
//!
//! ```text
//!     main thread:    NSApp.run() + reactive graph + UI
//!     compio worker:  Dispatcher's per-thread compio Runtime
//!                     (kqueue-driven via the `polling` crate)
//! ```
//!
//! ## What this is *not*
//!
//! Compio doesn't ship a high-level HTTP client. To keep deps
//! minimal we make a plain HTTP/1.1 GET ourselves over a raw
//! `compio::net::TcpStream` to a plaintext endpoint
//! (`icanhazip.com:80`). Real apps would pull in `cyper`, write
//! their own HTTPS via `compio-tls`, or just use tokio + reqwest.
//! The point here is the *integration*, not the HTTP surface.

extern crate leptos_cocoa as leptos_platform;

use compio::{
    buf::BufResult,
    dispatcher::Dispatcher,
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use leptos_platform::prelude::*;
use std::sync::Arc;

async fn fetch_ip_via_compio(d: Arc<Dispatcher>) -> Result<String, String> {
    // dispatch returns a futures_channel::oneshot::Receiver<R>;
    // R must be Send. The closure runs on a compio worker, where
    // CURRENT_RUNTIME is set, so compio::spawn / compio::net work
    // out of the box.
    let rx = d
        .dispatch(|| async {
            // Plain HTTP/1.1 GET. Endpoint returns the IP as the
            // entire body, one line, no JSON wrapping.
            let mut stream = TcpStream::connect("icanhazip.com:80")
                .await
                .map_err(|e| format!("connect: {e}"))?;
            let req = b"GET / HTTP/1.1\r\nHost: icanhazip.com\r\nConnection: close\r\n\r\n";
            // compio uses owned-buffer I/O: each call returns the
            // buffer along with the result, packed in a `BufResult`.
            let BufResult(res, _) = stream.write_all(req.to_vec()).await;
            res.map_err(|e| format!("write: {e}"))?;
            let BufResult(res, body) =
                stream.read_to_end(Vec::with_capacity(512)).await;
            res.map_err(|e| format!("read: {e}"))?;

            let body = String::from_utf8_lossy(&body).into_owned();
            // Split headers from body, return body trimmed.
            let ip = body
                .split_once("\r\n\r\n")
                .map(|(_, b)| b.trim().to_string())
                .ok_or_else(|| String::from("malformed response"))?;
            Ok::<String, String>(ip)
        })
        .map_err(|_| "dispatcher closed".to_string())?;
    // `rx` is a oneshot::Receiver. It's Send + Future, so our
    // main-thread spawner polls it. When the compio worker
    // finishes, the result lands here.
    rx.await.map_err(|_| "cancelled".to_string())?
}

#[component]
fn App(dispatcher: Arc<Dispatcher>) -> impl IntoView {
    let refresh = RwSignal::new(0u32);
    let ip = {
        let d = dispatcher.clone();
        AsyncDerived::new(move || {
            let _ = refresh.get();
            let d = d.clone();
            async move { fetch_ip_via_compio(d).await }
        })
    };

    view! {
        <vstack padding=20.0 gap=12.0>
            <label bold=true>"Public IP (via compio)"</label>
            <label>{move || match ip.get() {
                Some(Ok(ip))  => ip,
                Some(Err(e))  => format!("Error: {e}"),
                None          => "Fetching…".to_string(),
            }}</label>
            <button on:click=move |_| refresh.update(|n| *n += 1)>
                "Refresh"
            </button>
        </vstack>
    }
}

fn main() {
    // User owns the runtime construction, just like the tokio
    // examples. The Dispatcher spawns N=available_parallelism()
    // worker threads each running a compio runtime; we keep a
    // single Arc alive for the program's lifetime.
    let dispatcher = Arc::new(
        Dispatcher::new().expect("compio dispatcher"),
    );

    mount_to_window("ipify (compio)", (340.0, 160.0), {
        let d = dispatcher.clone();
        move || view! { <App dispatcher=d.clone() /> }
    }).run();
}
