# Working with Async

Native apps eventually want to do I/O — fetch an HTTP endpoint,
query a database, stream events over a websocket — without freezing
the UI. This chapter explains how to combine the leptos reactive
system with an async runtime like [tokio] in a way that keeps the
AppKit/UIKit/GTK run loop responsive.

The short version: you spin up tokio (or compio, or smol) on a
background thread, do your I/O there, and hand the result back to
the main thread to update signals. The framework stays out of the
way — *you* own the runtime, *you* call `tokio::spawn`. We provide
one tiny helper (`on_main`) for the push-back direction and call it
a day.

## The threading model

There are two executors in play:

**The main thread.** AppKit's `NSRunLoop`, libdispatch's main
queue, and the framework's reactive spawner are all the same
thing from your perspective: the single thread that owns the UI,
runs the reactive graph, and mutates signals. `NSApp.run()`
blocks this thread for the lifetime of the program.

**Your async runtime** — tokio, by default. Owns its own worker
thread(s) and its own reactor (mio + kqueue/epoll). I/O futures
from libraries like `reqwest`, `sqlx`, or `tonic` register with
tokio's reactor and *must* be polled on tokio's threads.

The two never share a thread. Tokio can't take over main —
`NSApp.run()` and `Runtime::block_on` are both non-cooperative
blocking calls. So tokio always lives on its own threads, and you
ferry data between them.

```text
  ┌──────────────────────────┐         ┌────────────────────────┐
  │  Main thread              │         │  Tokio worker thread   │
  │                          │         │                        │
  │  • NSApp.run()           │         │  • reqwest::get(...)   │
  │  • Reactive signals      │ ◀────── │  • tokio::spawn body   │
  │  • UI updates            │  result │  • TCP/TLS/mio reactor │
  │  • await JoinHandle ─────┼──spawn──▶                        │
  │                          │  fn     │                        │
  └──────────────────────────┘         └────────────────────────┘
```

## The load-bearing fact

`tokio::JoinHandle<T>` implements `Future<Output = Result<T, JoinError>>`,
and *polling it doesn't require a tokio context*. Only the inner
task needs to run on tokio's threads.

That single fact is what makes the whole integration clean. The
main thread can `.await` the result of work `tokio::spawn`'d onto
tokio, and tokio sees no main-thread weirdness because the polling
happens in its own scheduler.

```rust
async fn fetch_ip() -> Result<String, String> {
    // tokio::spawn — runs on a tokio worker thread.
    let handle = tokio::spawn(async {
        reqwest::get("https://api.ipify.org")
            .await
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())
    });
    // handle.await — polled by *our* main-thread spawner.
    handle.await.map_err(|e| e.to_string())?
}
```

## The four patterns

Most main↔tokio traffic falls into one of these four shapes. Pick
based on the *shape* of the work, not on what's easiest to type.

### 1. `tokio::spawn(fut).await`

The simplest pattern. Use it for one-shot I/O: HTTP requests, file
reads, single DB queries. Composes directly with `AsyncDerived`.

**Example:** [`cocoa/examples/ipify`](https://github.com/…/cocoa/examples/ipify)
fetches the public IP and displays it.

### 2. `tokio::spawn` + explicit `oneshot::channel`

Same capability as (1) but the spawned task outlives the caller, or
you want explicit cancellation, or you want to fan one result to
several consumers (use `broadcast` instead of `oneshot`). Drop the
receiver to silently cancel — the sender's `.send()` becomes a
no-op on the next poll.

**Example:** the "cancellable fetch" section in
[`cocoa/examples/async_patterns`](https://github.com/…/cocoa/examples/async_patterns).

### 3. Long-lived bidirectional `mpsc` pair

A persistent tokio task owns expensive state — a database
connection, a websocket, an authenticated gRPC stub. The UI sends
`Command`s in via `mpsc::Sender`; the task replies via a
per-request `oneshot::Sender`. The state lives in the tokio task
between requests.

**Example:** the "math service" section in `async_patterns`.

### 4. Direct cross-thread `set` from a worker

A tokio task generates a stream of values (websocket frames, SSE
events, periodic ticks) and pushes each one straight into a
signal — no channel, no `on_main` wrapping.

```rust
let count = RwSignal::new(0u64);
tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut n = 0u64;
    loop {
        tick.tick().await;
        n += 1;
        if count.try_set(n).is_some() {
            // Owner disposed (component unmounted) — shut down.
            break;
        }
    }
});
```

This works because `RwSignal<T, SyncStorage>` is `Send + Sync +
Copy` when `T: Send + Sync`, and its mutation API is thread-safe
by design. The notify cascade only flips atomic flags and wakes
wakers; the affected effect bodies are rescheduled onto the
framework's main-thread spawner (libdispatch on cocoa/iOS,
`MainContext::spawn_local` on GTK), and that's where the actual
UI work happens. No NSView / UIView / gtk widget call ever lands
off-main, because the spawner is the funnel.

`try_set` is the natural shutdown handshake. It returns
`Some(unwritten_value)` once the signal's `Owner` has disposed —
which is what happens on component unmount or window close. The
worker checks the return value and exits cleanly. Without it, a
tokio task can outlive the component it was driving and leak.

### When you still want `on_main`

For work that *isn't* a signal update — calling into native APIs
directly, running an arbitrary closure on the run loop — use
`on_main`:

```rust
use leptos_apple_shared::on_main;   // cocoa, iOS
// use gtk_dom::on_main;             // GTK

on_main(|| {
    // arbitrary work on the run-loop thread
});
```

`on_main` is a thin wrapper around libdispatch's main queue on
the Apple ports, and `glib::idle_add_once` on GTK. Same name,
same shape, same semantics. It's the only async-related helper
the framework ships.

**Example:** the "tick stream" section in `async_patterns`.

### 5. Eager cancellation via `on_cleanup` + `JoinHandle::abort`

Pattern 4 is *lazy*: the worker only learns it's unwanted on the
next attempted write. If the worker is blocked on a long `.await`
(slow HTTP request, 30 s timeout), that latency could be the
length of the whole operation.

For tasks where prompt cancellation matters, keep the
`tokio::task::JoinHandle` returned by `tokio::spawn` and abort it
when the component unmounts. `on_cleanup` runs at Owner-dispose
time; `JoinHandle::abort()` causes the task's next `.await` to
wake with cancellation and the task is dropped synchronously.

```rust
let task = RwSignal::new(None::<tokio::task::JoinHandle<()>>);
let handle = tokio::spawn(long_work);
task.set(Some(handle));

on_cleanup(move || {
    if let Some(h) = task.try_update(Option::take).flatten() {
        h.abort();
    }
});
```

See [Signals Across Threads](./signals.md) for the full pattern
and the broader signal/thread-safety story.

**Example:** the "eager cancel" section in `async_patterns`.

## What we don't provide

- No `spawn_io` / `spawn!` macro. `tokio::spawn(fut).await` is one
  line, well-documented in tokio's own docs, and composes with
  channels however you want.
- No automatic runtime construction. You write
  `let rt = tokio::runtime::Runtime::new()?; let _g = rt.enter();`
  in `main`. That's the *whole* framework integration.
- No tokio feature flag on `leptos_cocoa`. If you don't use tokio,
  the framework doesn't drag it in.

This is a library, not a framework: your code owns the runtime.
See the [Tokio](./tokio.md) page for runtime construction recipes,
including the current-thread variant. The
[Other Runtimes](./other_runtimes.md) page covers compio, smol, etc.
