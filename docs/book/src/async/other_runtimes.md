# Other Runtimes

The bridging pattern documented in the [Overview](./README.md) is
runtime-agnostic. Anything that:

1. Can be driven from a non-main thread (i.e. won't try to take
   over the AppKit/UIKit/GTK run loop), and
2. Can deliver results across a thread boundary (any `Send`
   channel works — `std::sync::mpsc`, `crossbeam`, `tokio::sync`,
   `futures_channel`),

…fits the same shape. The framework's main-thread spawner doesn't
care which runtime polls the off-main work.

## Compio

Compio is a completion-based runtime (io_uring on Linux, IOCP on
Windows, the `polling` crate on macOS) with a thread-per-core
architecture. Its API shape differs from tokio in one important
way: **`compio::Runtime` is `Rc`-based and `!Send`**. You can't
`enter()` a single runtime on multiple threads. So instead of
"one runtime, multi worker threads" (tokio's model), compio's
multi-threading story is "N independent runtimes, each on its own
thread, fed by a `Dispatcher`."

The cocoa main-thread executor doesn't notice the difference.

### Setting it up

```rust
use compio::dispatcher::Dispatcher;
use std::sync::Arc;

fn main() {
    // Dispatcher spawns N=available_parallelism() worker threads,
    // each running its own compio Runtime. Keep an Arc alive for
    // the life of the program.
    let dispatcher = Arc::new(
        Dispatcher::new().expect("compio dispatcher"),
    );

    mount_to_window("App", (320.0, 200.0), {
        let d = dispatcher.clone();
        move || view! { <App dispatcher=d.clone() /> }
    })
    .run();
}
```

No `_guard = rt.enter()` ceremony like tokio — compio doesn't use
thread-locals to resolve a "current runtime" the way tokio does.
The `Dispatcher` is just a typed channel into worker threads;
hand it around explicitly as a prop or via context.

### Dispatching work

```rust
async fn fetch_via_compio(d: Arc<Dispatcher>) -> Result<String, String> {
    let rx = d.dispatch(|| async {
        // Runs on a compio worker. CURRENT_RUNTIME is set here,
        // so compio::net / compio::fs / compio::spawn all work.
        let mut s = TcpStream::connect("example.com:80").await?;
        let BufResult(_, _) =
            s.write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec()).await;
        let BufResult(res, body) =
            s.read_to_end(Vec::with_capacity(512)).await;
        res?;
        Ok::<_, io::Error>(String::from_utf8_lossy(&body).into_owned())
    }).map_err(|_| "dispatcher closed".to_string())?;

    // rx is a futures_channel::oneshot::Receiver<R>. Send + Future.
    // Polled by our main-thread spawner; resolves when the compio
    // worker finishes.
    rx.await.map_err(|_| "cancelled".to_string())?
        .map_err(|e| e.to_string())
}
```

This is the exact same shape as the tokio pattern-1 example —
fire work onto an off-main runtime, await the result on main. The
only differences are cosmetic:

| concern | tokio | compio |
|---|---|---|
| **runtime context** | `rt.enter()` for life of program | dispatcher passed explicitly |
| **spawn API** | `tokio::spawn(fut)` (Send fut) | `dispatcher.dispatch(|| async { … })` (Send closure, non-Send Future) |
| **result bridge** | `JoinHandle<T>` (Send Future) | `oneshot::Receiver<T>` (Send Future) |
| **macOS reactor** | mio → kqueue | `polling` crate → kqueue |

### Caveats on macOS

- **No native HTTP client.** Compio doesn't ship a `reqwest`
  equivalent. Pull in `cyper`, write your own TLS via
  `compio-tls`, or accept plaintext HTTP. The framework's
  bridging story doesn't change either way — you just need
  *some* compio-aware Future to await.
- **`polling` crate fallback.** On macOS compio uses the
  `polling` crate (kqueue-backed but a portability layer, not
  raw kqueue). Performance is fine for typical app I/O; not
  io_uring-fast.
- **Thread-per-core.** `Dispatcher::new()` defaults to
  `available_parallelism()` worker threads. On a laptop GUI app
  this is overkill — use `Dispatcher::builder().worker_threads(1)
  .build()` for a single worker.

**Example:**
[`cocoa/examples/ipify_compio`](https://github.com/…/cocoa/examples/ipify_compio)
fetches the public IP via a plaintext HTTP/1.1 GET to
`icanhazip.com:80` over `compio::net::TcpStream`. Mirrors the
tokio `ipify` example in UX and structure; differs only in the
runtime construction and dispatch shape.

## smol / async-std / others

Same pattern. Construct the runtime in `main`, use its spawn API
to ferry work onto its threads, await the resulting future on
main. The framework only requires that the future you `.await`
on main is `Send + 'static` and pollable without that runtime's
thread-local context — same constraint as tokio's `JoinHandle`
and compio's `oneshot::Receiver`.

The `on_main` helper from `leptos_apple_shared` works regardless
of which runtime is on the other side. It's libdispatch on
macOS/iOS and (planned) GLib's main context on GTK — no tokio
involvement either way.

## GTK / GLib

GTK has its own async story: `glib::MainContext` can directly
spawn `!Send` futures, which is what the framework's own spawner
uses there. For *tokio-aware* I/O on GTK, the recommendation is
the same as for cocoa — tokio on its own threads, results piped
back via channel. The GTK version of `on_main` lives at
`gtk_dom::on_main` and wraps `glib::idle_add_once`. Same
signature as `leptos_apple_shared::on_main`, same call shape,
same semantics: schedule the closure on the next main-loop
tick from any thread.

```rust
// cocoa/iOS:
use leptos_apple_shared::on_main;

// GTK:
use gtk_dom::on_main;

// both:
on_main(|| { /* runs on the main loop thread */ });
```

**Example:**
[`gtk/examples/ipify`](https://github.com/…/gtk/examples/ipify)
and
[`gtk/examples/async_patterns`](https://github.com/…/gtk/examples/async_patterns)
are line-for-line mirrors of the cocoa equivalents, with only
the runtime entry point (`mount_to_window` signature) and the
`on_main` import path different.
