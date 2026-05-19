//! async_patterns — kitchen-sink demo of the four async ↔ main
//! bridging shapes documented in `docs/book/src/async/`.
//!
//! Each section in the window shows one pattern. Pattern 1 (the
//! simplest — `tokio::spawn(io).await`) lives in the separate
//! `ipify` / `placecats` examples; this app focuses on (2)–(4).
//!
//! **Pattern 2 — explicit oneshot, cancellable fetch.**
//! `tokio::spawn` runs the HTTP; we keep the oneshot `Receiver` on
//! main. Dropping the receiver before it resolves is our "cancel"
//! signal — the sender (in tokio) sees the channel close on its
//! next `tx.send(...)` and the result is silently discarded.
//!
//! **Pattern 3 — long-lived bidirectional channel pair.**
//! One tokio task owns a state machine (a math service that
//! remembers the last result). UI sends `Request`s in via mpsc;
//! the task replies through a per-request oneshot.
//!
//! **Pattern 4 — direct cross-thread `set` from a worker.**
//! A tokio task ticks every 500 ms and writes to a signal directly.
//! No `on_main`, no `SendWrapper` — `RwSignal<T, SyncStorage>` is
//! `Send + Sync + Copy`. Worker uses `try_set`'s return value as
//! a *lazy* disposal handshake.
//!
//! **Pattern 5 — eager cancellation via `on_cleanup` + `JoinHandle::abort`.**
//! Pattern 4 is lazy: the worker only learns it's unwanted on the
//! next tick (worst case: blocked on a long `.await`, never wakes).
//! For tasks where that latency matters (long HTTP requests, slow
//! DB queries), keep the `JoinHandle` and abort it on unmount.

use leptos_native::prelude::*;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

// ---------------------------------------------------------------------
// Pattern 2: cancellable fetch via explicit oneshot
// ---------------------------------------------------------------------

#[component]
fn CancellableFetch() -> impl IntoView {
    let status = RwSignal::new(String::from("idle"));
    // Hold the receiver here so dropping it cancels the in-flight
    // request. `SendWrapper` because RwSignal is !Send and we
    // capture the cancel slot via signal indirectly.
    let cancel_slot: RwSignal<Option<oneshot::Receiver<String>>> =
        RwSignal::new(None);

    let start = move |_| {
        status.set("fetching…".into());
        let (tx, rx) = oneshot::channel::<String>();
        // Replace the slot — dropping any previous Receiver
        // implicitly cancels its sender side.
        cancel_slot.set(Some(rx));

        tokio::spawn(async move {
            // Slow endpoint: ipify with an artificial delay
            // before reporting back, so cancel is observable.
            tokio::time::sleep(Duration::from_secs(2)).await;
            let body = match reqwest::get("https://api.ipify.org")
                .await
            {
                Ok(r) => r.text().await.unwrap_or_else(|e| e.to_string()),
                Err(e) => e.to_string(),
            };
            // If the receiver was dropped (Cancel button), this
            // send silently fails — that's the cancellation.
            let _ = tx.send(body);
        });

        // Spawn a main-thread task to await the receiver. If the
        // slot still holds it when the result arrives, publish;
        // otherwise we were cancelled.
        leptos_native::core::task::spawn_local(async move {
            // Take the current receiver. If it's been replaced
            // already (e.g. user clicked Start twice in a row),
            // this drops the previous one — which is fine; we
            // bail out and the newer task takes over.
            let Some(rx) = cancel_slot.try_update(Option::take).flatten() else {
                return;
            };
            match rx.await {
                Ok(body) => status.set(format!("result: {}", body.trim())),
                Err(_) => status.set("cancelled".into()),
            }
        });
    };

    let cancel = move |_| {
        // Drop the receiver → sender's `.send()` becomes a no-op,
        // the main-thread await wakes with `Err(Canceled)`.
        cancel_slot.set(None);
        status.set("cancelled".into());
    };

    view! {
        <vstack gap=4.0>
            <label bold=true>"Pattern 2 — cancellable fetch"</label>
            <label>{move || status.get()}</label>
            <hstack gap=8.0>
                <button on:click=start>"Start"</button>
                <button on:click=cancel>"Cancel"</button>
            </hstack>
        </vstack>
    }
}

// ---------------------------------------------------------------------
// Pattern 3: long-lived bidirectional channel (a "math service")
// ---------------------------------------------------------------------

enum Op { Add, Mul }

struct MathRequest {
    op: Op,
    a: i64,
    b: i64,
    reply: oneshot::Sender<i64>,
}

/// Spawn a tokio task that owns the math service state. Returns
/// the request sender; the task lives until the sender drops, at
/// which point its receiver yields `None` and the loop exits.
fn spawn_math_service() -> mpsc::UnboundedSender<MathRequest> {
    let (tx, mut rx) = mpsc::unbounded_channel::<MathRequest>();
    tokio::spawn(async move {
        // Pretend there's some expensive per-request setup here
        // (a DB connection, an HTTP client, a model handle…).
        let mut total_requests = 0u64;
        while let Some(req) = rx.recv().await {
            total_requests += 1;
            let result = match req.op {
                Op::Add => req.a.wrapping_add(req.b),
                Op::Mul => req.a.wrapping_mul(req.b),
            };
            // Best-effort: caller may have dropped the receiver.
            let _ = req.reply.send(result);
            // Drop-in spot for state that survives across requests.
            let _ = total_requests;
        }
    });
    tx
}

#[component]
fn MathService() -> impl IntoView {
    // The mpsc::UnboundedSender is Send + Clone; we just stash it
    // in the component's local scope.
    let svc = spawn_math_service();
    let last = RwSignal::new(String::from("(no requests yet)"));

    let call = move |op: Op, a: i64, b: i64| {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = svc.send(MathRequest { op, a, b, reply: reply_tx });
        leptos_native::core::task::spawn_local(async move {
            if let Ok(v) = reply_rx.await {
                last.set(format!("= {v}"));
            }
        });
    };

    let call_add = call.clone();
    let call_mul = call.clone();
    let call_big = call.clone();

    view! {
        <vstack gap=4.0>
            <label bold=true>"Pattern 3 — persistent worker task"</label>
            <label>{move || last.get()}</label>
            <hstack gap=8.0>
                <button on:click=move |_| call_add(Op::Add, 3, 4)>
                    "3 + 4"
                </button>
                <button on:click=move |_| call_mul(Op::Mul, 6, 7)>
                    "6 × 7"
                </button>
                <button on:click=move |_| call_big(Op::Add, 1_000_000, 23)>
                    "big + 23"
                </button>
            </hstack>
        </vstack>
    }
}

// ---------------------------------------------------------------------
// Pattern 4: direct cross-thread set with disposal-driven shutdown
// ---------------------------------------------------------------------
//
// `RwSignal<T, SyncStorage>` is `Send + Sync + Copy` when
// `T: Send + Sync`, and its mutation primitives (`set`/`update`/
// `try_set`) are thread-safe — the notify cascade only flips atomic
// flags and wakes wakers that ultimately re-dispatch the affected
// effect bodies onto the main thread via libdispatch. NSView is
// never touched off-main.
//
// So the worker can just call `count.try_set(n)` directly. The
// return value is the shutdown signal: `Some(n)` means the signal
// was already disposed (its Owner unmounted), so the worker should
// stop. `None` means the write landed.

#[component]
fn TickStream() -> impl IntoView {
    let count = RwSignal::new(0u64);
    // Bump on each mount so the logs make it obvious that a new
    // worker started (i.e. we got a fresh component scope).
    static INSTANCE: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);
    let instance =
        INSTANCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    eprintln!("[tick #{instance}] spawning worker");

    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(500));
        let mut n = 0u64;
        loop {
            tick.tick().await;
            n += 1;
            if count.try_set(n).is_some() {
                // Signal disposed — component unmounted. Quit.
                eprintln!(
                    "[tick #{instance}] signal disposed at n={n}; \
                     worker exiting"
                );
                break;
            }
        }
    });

    view! {
        <vstack gap=4.0>
            <label bold=true>"Pattern 4 — direct set from a worker"</label>
            <label>{move || format!("ticks: {} (instance #{instance})",
                count.get())}</label>
        </vstack>
    }
}

// ---------------------------------------------------------------------
// Pattern 5: eager cancellation with on_cleanup + JoinHandle::abort
// ---------------------------------------------------------------------
//
// Pattern 4's lazy shutdown (worker checks `try_set` return value)
// is fine for tight loops, but breaks down for tasks that sit on a
// long `.await` — the cancellation can't fire until the await
// completes. For example, `reqwest::get` of a slow endpoint, a 30 s
// timeout, a websocket frame read. The worker would happily finish
// the work before noticing the signal is gone.
//
// Eager fix: stash the `JoinHandle` somewhere we can reach during
// unmount, and call `abort()` on it. `abort()` causes the next
// `.await` inside the task to wake with cancellation; the task is
// then dropped synchronously (its captured state goes away).
//
// `on_cleanup` runs when the component's `Owner` disposes — before
// the arena drops the signals — so we can read the JoinHandle out
// of the signal at that point.

#[component]
fn EagerCancel() -> impl IntoView {
    let status = RwSignal::new(String::from("idle"));
    // Slot for the in-flight task's handle. JoinHandle<()> is
    // Send + Sync; this signal is therefore Send + Sync too.
    let task_slot =
        RwSignal::new(None::<tokio::task::JoinHandle<()>>);

    let start = move |_| {
        // Cancel any previous run before starting a new one.
        if let Some(prev) = task_slot.try_update(Option::take).flatten() {
            prev.abort();
        }
        status.set("running (10s)…".into());
        eprintln!("[eager_cancel] starting 10s task");

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            // If we get here, we weren't aborted. try_set won't
            // notify subscribers if the signal has already
            // disposed.
            let _ = status.try_set("done!".into());
            eprintln!("[eager_cancel] task finished naturally");
        });
        task_slot.set(Some(handle));
    };

    let cancel = move |_| {
        if let Some(h) = task_slot.try_update(Option::take).flatten() {
            h.abort();
            status.set("cancelled".into());
            eprintln!("[eager_cancel] user cancelled");
        }
    };

    // Eagerly cancel on component unmount. Without this, hiding
    // the EagerCancel component (e.g. via <Show>) leaves the 10 s
    // tokio task running until its sleep finishes — that work is
    // wasted, and any state captured in the closure stays alive
    // alongside it.
    on_cleanup(move || {
        if let Some(h) = task_slot.try_update(Option::take).flatten() {
            h.abort();
            eprintln!("[eager_cancel] aborted in-flight task on unmount");
        }
    });

    view! {
        <vstack gap=4.0>
            <label bold=true>"Pattern 5 — eager cancellation via abort()"</label>
            <label>{move || status.get()}</label>
            <hstack gap=8.0>
                <button on:click=start>"Start (10s)"</button>
                <button on:click=cancel>"Cancel"</button>
            </hstack>
        </vstack>
    }
}

// ---------------------------------------------------------------------
// Top-level
// ---------------------------------------------------------------------

#[component]
fn App() -> impl IntoView {
    // Hide/show the bottom two patterns to prove the disposal-
    // shutdown story works: untick this and watch stderr for the
    // worker-exit / abort-on-unmount log lines. Re-tick and the
    // workers respawn from scratch.
    let show_tick = RwSignal::new(true);

    view! {
        <vstack padding=20.0 gap=20.0>
            <CancellableFetch/>
            <MathService/>

            <checkbox bind:checked=show_tick>
                "Show tick stream + eager-cancel demo \
                 (untick to dispose their signals)"
            </checkbox>
            <Show
                when=move || show_tick.get()
                fallback=|| view! {
                    <label text_color=Color::GRAY>
                        "(workers have shut down — start an EagerCancel \
                          run, then untick, watch stderr)"
                    </label>
                }>
                <vstack gap=20.0>
                    <TickStream/>
                    <EagerCancel/>
                </vstack>
            </Show>
        </vstack>
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let _guard = rt.enter();

    mount_to_window("async patterns", (440.0, 380.0), || view! { <App /> }).run();
}
