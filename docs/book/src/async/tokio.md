# Tokio

Tokio is the de-facto async runtime for Rust networking and
filesystem I/O. Most async libraries (reqwest, sqlx, hyper, tonic,
redis with `tokio-comp`) pin themselves to tokio specifically — if
you want one of those, you want a tokio runtime alive somewhere.

## Setting up the runtime

```rust
use leptos::prelude::*;

fn main() {
    // 1. Construct the runtime.
    let rt = tokio::runtime::Runtime::new()
        .expect("tokio runtime");

    // 2. Enter it on the main thread, so tokio::spawn(...) called
    //    from our main-thread reactive futures knows which runtime
    //    to target. `_guard` must outlive every spawn call.
    let _guard = rt.enter();

    // 3. Run the app. NSApp.run() blocks here for the lifetime of
    //    the program; the guard above is kept alive in the same
    //    stack frame, so the runtime context stays set on main.
    mount_to_window("My App", (320.0, 240.0), || view! { <App /> });
}
```

The Cargo dependency:

```toml
[dependencies]
leptos = { package = "leptos_cocoa", path = "..." }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
# Plus whatever you actually use I/O-wise:
reqwest = { version = "0.12", default-features = false,
            features = ["rustls-tls", "json"] }
```

Use `rustls-tls` instead of the default `native-tls` to avoid
having to link the system OpenSSL — it just works on macOS.

## Multi-threaded vs current-thread

Tokio has two schedulers. **They look identical at the call site.**
The choice is about resource footprint, not API.

### Multi-threaded (default)

```rust
let rt = tokio::runtime::Runtime::new()?;
```

`Runtime::new()` is shorthand for `Builder::new_multi_thread()
.enable_all().build()`. Workers drive the reactor on their own —
nothing more to do. This is the right default.

When you'd reach for it: any non-trivial app. Lots of concurrent
I/O. Doesn't care about CPU footprint.

### Current-thread

```rust
use std::future;

let rt = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

// `current_thread` runs all tasks on one thread. We can't tick it
// from main (AppKit owns that), so park it on a side thread:
let handle = rt.handle().clone();
std::thread::Builder::new()
    .name("tokio-current-thread".into())
    .spawn(move || rt.block_on(future::pending::<()>()))
    .expect("spawn tokio driver thread");

// On main, enter the handle so tokio::spawn(...) resolves to this
// runtime via thread-local. The guard outlives mount_to_window.
let _guard = handle.enter();
```

When you'd reach for it: low-resource apps (one I/O thread instead
of N=CPU-count). Tests that want deterministic single-threaded
ordering. Embedded contexts.

The footgun this avoids: you might think *"current_thread → just
run it on the main thread."* That doesn't work — `rt.enter()`
alone only sets thread-locals; it doesn't drive the scheduler.
Without `rt.block_on(...)` running somewhere, your `tokio::spawn`
tasks never make progress. The side-thread park is the standard
workaround.

**Example:** [`cocoa/examples/ipify_current_thread`](https://github.com/…/cocoa/examples/ipify_current_thread).

## Awaiting tokio results from main

The framework's main-thread spawner can poll any `Send + 'static`
future. `tokio::spawn` returns a `JoinHandle<T>` that fits that
bound and doesn't need a tokio context to poll — it's just a
oneshot-like completion signal.

```rust
async fn do_io() -> Result<Body, Error> {
    let handle = tokio::spawn(async {
        reqwest::get("…").await?.bytes().await
    });
    handle.await
        .map_err(Into::into)        // JoinError
        .and_then(|r| r.map_err(Into::into))  // reqwest::Error
}
```

Inside `AsyncDerived::new(|| async { do_io().await })`:

```rust
let data = AsyncDerived::new(|| async {
    do_io().await
});

view! {
    <label>{move || match data.get() {
        Some(Ok(_))  => "✓".into(),
        Some(Err(e)) => format!("error: {e}"),
        None         => "loading…".into(),
    }}</label>
}
```

`data.get()` reads as `Option<T>` — `None` while the future is
in-flight, `Some(value)` once it resolves. The view re-renders
automatically.

## The `Send` boundary

`tokio::spawn` requires `Send + 'static` futures. Most reactive
types (`RwSignal`, `WriteSignal`, etc.) are `!Send` because they
contain `Rc`s into the reactive graph — they only make sense on
the thread that owns the graph (main).

If you need to write to a signal *from* a tokio task (pattern 4,
the `on_main` push), wrap the signal in `send_wrapper::SendWrapper`
before moving it into the `tokio::spawn` closure. `SendWrapper`
panics at runtime if accessed off its origin thread — and the
`on_main` callback runs on main, so it's fine.

```rust
use send_wrapper::SendWrapper;

let sig = SendWrapper::new(my_signal);
tokio::spawn(async move {
    let v = compute_something().await;
    let sig = sig.clone();
    leptos_apple_shared::on_main(move || sig.set(v));
});
```

For patterns 1–3, you don't need `SendWrapper` because signals are
only touched on main — the tokio side only sees the inputs (which
you've already copied/cloned) and produces a result that flows back
via channel.

## What about `tokio::time::sleep` without networking?

Same rules. `tokio::time::sleep` registers with tokio's time
driver, which needs to be alive on a tokio worker. Multi-thread
just works. For current-thread, the side-thread park drives it.

```rust
let result = AsyncDerived::new(|| async {
    tokio::time::sleep(Duration::from_secs(1)).await;
    42
});
```

The `sleep` future is `Send`, so you can `.await` it from main
directly (the time driver wakes the future from a tokio worker,
the framework's main-thread spawner polls it). No `tokio::spawn`
needed for plain timers.

## Gotchas

- **Drop the `_guard` and `tokio::spawn` panics.** Don't shadow it,
  don't `let _ = rt.enter();` (that drops immediately).
- **`Runtime::new()` from inside `tokio::spawn` panics.** Construct
  the runtime in `main`, never re-entrantly.
- **`Builder::new_current_thread()` without `enable_all()` (or
  `enable_io()` / `enable_time()`) silently won't drive I/O or
  timers.** Always include `enable_all()` unless you know exactly
  which drivers you need.
- **`reqwest` with the `default-tls` feature pulls in the system
  TLS stack and slows builds.** Use `default-features = false,
  features = ["rustls-tls"]` instead.
