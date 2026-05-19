# Signals across threads

The four bridging patterns in the [Overview](./README.md) all hinge
on one property: **reactive signals are thread-safe**. You can move
an `RwSignal<T>` into a `tokio::spawn` closure and call `.set()`
from a worker. Subscribers on the main thread will see the change
and re-render — and they'll do that re-render on the main thread,
not on the worker.

This page documents how that works, when it doesn't apply, and how
to clean up after yourself.

## What's `Send`, what's not

`RwSignal<T, S = SyncStorage>` is `Send + Sync + Copy` **when
`T: Send + Sync`**. The default `SyncStorage` mode requires
`T: Send + Sync + 'static`. Most ordinary Rust types (numbers,
strings, `Vec`, `Option`, your own structs of `Send + Sync` fields)
satisfy this trivially.

For `!Send` types you can use `RwSignal<T, LocalStorage>`, which is
*not* thread-safe. The framework also uses `LocalStorage`
internally for some `!Send` things, and `LocalStorage` signals
*panic* if accessed off-thread. They're the opt-out from the
multi-thread model — useful when wrapping a UI handle or a `Rc`,
not what you want for app-level state.

## `RwSignal` vs `ArcRwSignal`

There are two flavours of read/write signal:

**`ArcRwSignal<T>`** owns its data directly via
`Arc<RwLock<T>>` and `Arc<RwLock<SubscriberSet>>`. It's
`Send + Sync + Clone` (Arc clone is cheap), and its lifetime is
exactly the lifetime of its last clone — independent of any
component. Created with `ArcRwSignal::new(initial)`.

**`RwSignal<T>`** is a `Copy` token (`ArenaItem<ArcRwSignal<T>, _>`)
into the current reactive `Owner`'s arena. The arena holds the
underlying `ArcRwSignal` for you. The signal lives as long as the
Owner does, and is **dropped from the arena when the Owner
disposes** — i.e. when the component that created it unmounts.
Created with `RwSignal::new(initial)` inside a component.

Trade-offs:

| | `RwSignal<T>` | `ArcRwSignal<T>` |
|---|---|---|
| API | `Copy` — capture by value freely | `Clone` — cheap Arc bump |
| Lifetime | Tied to the creating component's Owner | Until last clone drops |
| `.set()` after dispose | Silent no-op (with debug warning), `try_set` returns `Some(unwritten)` | Always succeeds; subscribers may have already been disposed |
| `Send + Sync` | When `T: Send + Sync` | When `T: Send + Sync` |

Most application code uses `RwSignal` because it's `Copy` and
disposes cleanly with the component. Use `ArcRwSignal` when you
specifically want signal storage that outlives a component, or when
sharing a signal between modules without going through context.

You can convert between them in either direction:

```rust
// Lift an ArcRwSignal into the current Owner's arena:
let rw: RwSignal<T> = arc_sig.clone().into();

// Pull the ArcRwSignal back out:
let arc: ArcRwSignal<T> = rw.into();   // panics if disposed
```

## Disposal is your shutdown signal

`RwSignal::try_set` returns `Option<T>`:

- `None` — the write landed; subscribers were notified.
- `Some(unwritten)` — the signal's Owner has disposed; the value
  you handed in comes back to you.

The "disposed" branch is the cue: the component that owns this
signal no longer exists; you should stop producing values for it.

```rust
let count = RwSignal::new(0u64);

tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut n = 0u64;
    loop {
        tick.tick().await;
        n += 1;
        if count.try_set(n).is_some() {
            // Owner disposed; component unmounted. Stop the worker.
            break;
        }
    }
});
```

This is the **lazy** shutdown story: the worker doesn't know it's
unwanted until its next attempted write. That's fine for tight
loops (a 500 ms timer notices within 500 ms), but breaks down for
long awaits.

## Why this is sound

When you call `.set()` from a worker thread, the notification
cascade that follows — `mark_dirty`, `mark_subscribers_check`,
`Notify::notify` on each subscriber, the waker dance — uses only
`Send + Sync` primitives (`std::sync::RwLock`, `AtomicBool`,
`AtomicWaker`). Nothing touches UI state, nothing enters a
thread-local that wasn't designed for cross-thread access.

The **effect bodies** (the closures that actually mutate UI views
in response to a signal change) are *not* invoked directly from
the worker. They're polled by the framework's main-thread
spawner: each effect's `Wake` impl dispatches a poll onto
`DispatchQueue::main()` (cocoa/iOS) or `MainContext::spawn_local`
(GTK), and the effect re-runs there. So a write from any thread
results in a UI update on the main thread — automatically.

This is why we can hand an `RwSignal` to tokio without
`SendWrapper`, `thread_local!`, or any marshalling helper. The
spawner is the funnel.

## Eager cancellation for long-running tasks

The lazy `try_set` pattern only works if the worker periodically
attempts a write. A task blocked on a long single `.await` — a 30 s
HTTP request, a websocket frame read, a slow DB query — won't
notice that the user navigated away until its current await
completes. That's wasted work, possibly resource leaks, and (if it
finally writes back) a confused user.

The fix: **keep the `JoinHandle` and abort it on unmount.**
`tokio::task::JoinHandle::abort()` causes the task's next `.await`
to wake with a cancellation error; the task is dropped immediately,
and everything it captured drops with it.

`leptos_native::prelude::on_cleanup` registers a closure to run when the
current component's Owner disposes — that's the unmount hook we
need:

```rust
use std::time::Duration;

#[component]
fn EagerCancel() -> impl IntoView {
    let status = RwSignal::new(String::from("idle"));
    // Slot for the in-flight task's handle. JoinHandle<()> is
    // Send + Sync, so this signal is too.
    let task_slot = RwSignal::new(None::<tokio::task::JoinHandle<()>>);

    let start = move |_| {
        // Cancel any previous run before starting a new one.
        if let Some(prev) = task_slot.try_update(Option::take).flatten() {
            prev.abort();
        }
        status.set("running…".into());
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

    // Eagerly cancel when the component unmounts.
    on_cleanup(move || {
        if let Some(h) = task_slot.try_update(Option::take).flatten() {
            h.abort();
        }
    });

    view! {
        <vstack gap=4.0>
            <label>{move || status.get()}</label>
            <button on:click=start>"Start (10s)"</button>
            <button on:click=cancel>"Cancel"</button>
        </vstack>
    }
}
```

Three benefits over the lazy pattern:

1. **No wasted work.** The 10 s sleep doesn't keep ticking on the
   tokio runtime after the user navigates away.
2. **Captures drop immediately.** Anything the spawned async block
   captured (open sockets, large buffers, in-flight DB connections)
   releases as soon as the task drops.
3. **User-visible cancel button.** The same `abort()` call powers
   both the in-component Cancel button and the on-unmount cleanup.

`on_cleanup` runs *before* the arena drops the signals, so reading
the `JoinHandle` out of `task_slot` inside the cleanup closure is
safe — the signal's storage is still alive at that point.

**Example:** the `EagerCancel` section in
[`cocoa/examples/async_patterns`](https://github.com/…/cocoa/examples/async_patterns)
(also mirrored in the iOS and GTK ports). The example's `AUTO_TOGGLE=1`
env-var harness flips the parent `<Show>` checkbox on a timer so
you can watch the abort-on-unmount log lines fire without
clicking around.

## When you'd use what

| Situation | Pattern |
|---|---|
| Short tight-loop worker (ticker, polling) | Pattern 4: lazy `try_set` |
| One-shot fetch backing a `Resource` / `AsyncDerived` | Pattern 1: `tokio::spawn(io).await` |
| Need to cancel mid-flight by user action | Pattern 2: oneshot + drop |
| Multiple operations sharing a stateful worker | Pattern 3: long-lived mpsc |
| Long single-await that should abort on unmount | Pattern 5: `on_cleanup` + `JoinHandle::abort` |

(Patterns 1–4 are documented in the [Overview](./README.md);
pattern 5 here.)

## Concurrent reads and writes

`RwSignal::set` from a worker briefly holds the inner value's
`std::sync::RwLock` for write. If the main thread is reading the
same signal at the same moment (inside a view's `move || sig.get()`
closure), one will block the other for the duration of the write —
usually microseconds. This is standard `RwLock` semantics: safe,
but a synchronous wait.

In practice this is a non-issue because:

- Worker writes are rare (a few times per second for a tick stream,
  once per request for a fetch).
- The value clone / replace inside `set` is fast.
- Effects on main run *after* the write completes, so the visible
  "tearing" you might fear can't happen — the effect is polled in a
  later main-loop tick, and by then the write is long done.

Reading a signal *from a worker thread* is technically sound (the
`RwLock` handles concurrent reads), but it's not part of the
reactive contract — the read won't register a subscription, and you
get a snapshot that may be stale by the time you observe it. If you
need a worker-side view of a signal, shadow it in worker-local
state and update it via a channel.
