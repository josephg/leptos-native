//! iOS mirror of `cocoa/examples/async_patterns`. Same four
//! patterns, same `on_main` helper (libdispatch's main queue is
//! identical between macOS and iOS), same `thread_local!`
//! workaround for the !Send signal in pattern 4.
//!
//! See the cocoa version for prose; the only structural change
//! here is the iOS-style `#[cfg(target_os = "ios")] mod app`
//! wrapping that lets the file participate in workspace metadata
//! checks (which run on the host) without the iOS deps being
//! resolved.

extern crate leptos_uikit as leptos_platform;

#[cfg(target_os = "ios")]
mod app {
    use leptos_platform::prelude::*;
    use std::time::Duration;
    use tokio::sync::{mpsc, oneshot};

    // -----------------------------------------------------------------
    // Pattern 2: cancellable fetch
    // -----------------------------------------------------------------

    #[component]
    fn CancellableFetch() -> impl IntoView {
        let status = RwSignal::new(String::from("idle"));
        let cancel_slot: RwSignal<Option<oneshot::Receiver<String>>> =
            RwSignal::new(None);

        let start = move |_| {
            status.set("fetching…".into());
            let (tx, rx) = oneshot::channel::<String>();
            cancel_slot.set(Some(rx));
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let body = match reqwest::get("https://api.ipify.org").await {
                    Ok(r) => r.text().await.unwrap_or_else(|e| e.to_string()),
                    Err(e) => e.to_string(),
                };
                let _ = tx.send(body);
            });
            leptos_platform::core::task::spawn_local(async move {
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
            cancel_slot.set(None);
            status.set("cancelled".into());
        };

        view! {
            <vstack gap=4.0>
                <label>"Pattern 2 — cancellable fetch"</label>
                <label>{move || status.get()}</label>
                <hstack gap=8.0>
                    <button on:click=start>"Start"</button>
                    <button on:click=cancel>"Cancel"</button>
                </hstack>
            </vstack>
        }
    }

    // -----------------------------------------------------------------
    // Pattern 3: persistent worker
    // -----------------------------------------------------------------

    enum Op { Add, Mul }

    struct MathRequest {
        op: Op,
        a: i64,
        b: i64,
        reply: oneshot::Sender<i64>,
    }

    fn spawn_math_service() -> mpsc::UnboundedSender<MathRequest> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MathRequest>();
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let result = match req.op {
                    Op::Add => req.a.wrapping_add(req.b),
                    Op::Mul => req.a.wrapping_mul(req.b),
                };
                let _ = req.reply.send(result);
            }
        });
        tx
    }

    #[component]
    fn MathService() -> impl IntoView {
        let svc = spawn_math_service();
        let last = RwSignal::new(String::from("(no requests yet)"));

        let call = move |op: Op, a: i64, b: i64| {
            let (reply_tx, reply_rx) = oneshot::channel();
            let _ = svc.send(MathRequest { op, a, b, reply: reply_tx });
            leptos_platform::core::task::spawn_local(async move {
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
                <label>"Pattern 3 — persistent worker"</label>
                <label>{move || last.get()}</label>
                <hstack gap=8.0>
                    <button on:click=move |_| call_add(Op::Add, 3, 4)>"3 + 4"</button>
                    <button on:click=move |_| call_mul(Op::Mul, 6, 7)>"6 × 7"</button>
                    <button on:click=move |_| call_big(Op::Add, 1_000_000, 23)>"big"</button>
                </hstack>
            </vstack>
        }
    }

    // -----------------------------------------------------------------
    // Pattern 4: direct cross-thread set with disposal shutdown
    // -----------------------------------------------------------------
    //
    // RwSignal<T, SyncStorage>'s mutation API is thread-safe; notify
    // only flips atomic flags and reschedules effect bodies onto the
    // framework's main-thread spawner (libdispatch on iOS, same as
    // cocoa). `try_set` returns `Some(unwritten)` if the signal's
    // Owner has disposed — natural worker-shutdown signal.

    #[component]
    fn TickStream() -> impl IntoView {
        let count = RwSignal::new(0u64);
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
                <label>"Pattern 4 — direct set from a worker"</label>
                <label>{move || format!("ticks: {} (instance #{instance})",
                    count.get())}</label>
            </vstack>
        }
    }

    // -----------------------------------------------------------------
    // Pattern 5: eager cancellation with on_cleanup + abort
    // -----------------------------------------------------------------

    #[component]
    fn EagerCancel() -> impl IntoView {
        let status = RwSignal::new(String::from("idle"));
        let task_slot =
            RwSignal::new(None::<tokio::task::JoinHandle<()>>);

        let start = move |_| {
            if let Some(prev) = task_slot.try_update(Option::take).flatten() {
                prev.abort();
            }
            status.set("running (10s)…".into());
            let handle = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let _ = status.try_set("done!".into());
            });
            task_slot.set(Some(handle));
        };

        let cancel = move |_| {
            if let Some(h) = task_slot.try_update(Option::take).flatten() {
                h.abort();
                status.set("cancelled".into());
            }
        };

        // Eager-cancel on unmount: abort the in-flight task so its
        // 10 s sleep doesn't keep running unowned.
        on_cleanup(move || {
            if let Some(h) = task_slot.try_update(Option::take).flatten() {
                h.abort();
                eprintln!("[eager_cancel] aborted in-flight task on unmount");
            }
        });

        view! {
            <vstack gap=4.0>
                <label>"Pattern 5 — eager cancellation via abort()"</label>
                <label>{move || status.get()}</label>
                <hstack gap=8.0>
                    <button on:click=start>"Start (10s)"</button>
                    <button on:click=cancel>"Cancel"</button>
                </hstack>
            </vstack>
        }
    }

    // -----------------------------------------------------------------
    // Top-level
    // -----------------------------------------------------------------

    #[component]
    fn App() -> impl IntoView {
        // Toggle to prove the disposal-shutdown story works:
        // untick this, watch stderr for the worker exit line;
        // re-tick and a fresh instance starts from zero.
        let show_tick = RwSignal::new(true);

        view! {
            <vstack padding=20.0 gap=20.0>
                <CancellableFetch/>
                <MathService/>

                <hstack gap=8.0>
                    <switch bind:checked=show_tick/>
                    <label>"Show tick stream"</label>
                </hstack>
                <Show
                    when=move || show_tick.get()
                    fallback=|| view! {
                        <label>"(workers shut down)"</label>
                    }>
                    <vstack gap=20.0>
                        <TickStream/>
                        <EagerCancel/>
                    </vstack>
                </Show>
            </vstack>
        }
    }

    pub fn main() {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let _guard = rt.enter();
        leptos_platform::mount_ios::run(|| view! { <App /> });
    }
}

#[cfg(target_os = "ios")]
fn main() { app::main() }

#[cfg(not(target_os = "ios"))]
fn main() {}
