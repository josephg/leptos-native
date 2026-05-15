# Cross-thread signal mutation — design notes

Captured 2026-05-15 during the tokio-integration work.

## 2026-05-15 — Resolution

**Direct cross-thread `signal.set()` is safe and is now the
recommended pattern.** Audited the `Notify` chain end-to-end
(see `implementation_log.md` and the trace below) and confirmed
every operation on the worker-thread path uses standard
thread-safe primitives only — `std::sync::RwLock`,
`AtomicBool`, `futures::task::AtomicWaker`, and atomic Arc
operations. Nothing touches the `OWNER` thread-local, nothing
recursively re-enters the arena lock, and crucially the
*effect bodies* (the only code that touches NSView / UIView /
gtk widgets) only ever run inside a poll driven by the
framework's main-thread spawner. The spawner is the funnel:
off-main code can only flip flags and wake wakers; the wakers
translate to `dispatch_async` / `idle_add_once`, which
schedule the effect re-runs back onto main.

### The audited chain

`worker: ArcRwSignal::set(v)` →
`WriteGuard::drop` → `Notify::notify` →
`mark_dirty` → `mark_subscribers_check` (clones SubscriberSet
under read lock, iterates) → for each subscriber, e.g.
`EffectInner::mark_dirty` (write-lock, set dirty=true,
`Sender::notify`) → `AtomicWaker::wake` → invokes the registered
waker → spawner's `wake_by_ref` does
`DispatchQueue::main().exec_async(poll_main)` / equivalent →
**main thread later runs `poll_main`** → polls the spawn_local'd
task → calls `update_if_necessary` → runs the user's effect body
(NSView calls happen here, on main).

### `try_set` as the disposal handshake

`RwSignal::try_set(v)` returns `Some(unwritten_value)` if the
signal's `Owner` has disposed, `None` if the write landed.
Workers use this as a natural shutdown signal:

```rust
let count = RwSignal::new(0u64);
tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    let mut n = 0u64;
    loop {
        tick.tick().await;
        n += 1;
        if count.try_set(n).is_some() { break; }
    }
});
```

`RwSignal<T, SyncStorage>` is `Send + Sync + Copy` when
`T: Send + Sync`, so the closure captures the signal directly —
no `SendWrapper`, no `thread_local!`, no `on_main` wrapper for
this case. The `async_patterns` example's TickStream sections
on cocoa, iOS, and GTK all use this pattern as of 2026-05-15.

### Why `RemoteSignal<T>` wasn't built

Earlier drafts of this document proposed a `RemoteSignal<T>`
wrapper around `ArcRwSignal<T>`. With the audit complete, the
wrapper has no purpose:

- `ArcRwSignal<T>` is already `Send + Sync + Clone` (when
  `T: Send + Sync`) and its `.set` is thread-safe.
- `RwSignal<T>` is already `Send + Sync + Copy` (when
  `T: Send + Sync`) and its `.set` / `.try_set` are thread-safe.
- Re-exporting either as a new type just adds vocabulary.

The book chapter documents `RwSignal::try_set` directly as the
canonical pattern-4 idiom.

### `on_main` is still useful for non-signal work

`leptos_apple_shared::on_main` (cocoa/iOS) and `gtk_dom::on_main`
(GTK) stay in the framework for the case where you want to run
an arbitrary closure on the run loop without going through a
signal (calling native API directly, mutating a non-reactive
data structure visible to the main thread, etc.).

### Caveats that survive

- `T: Send + Sync` is required (enforced by `SyncStorage`'s
  `Storage<T>` bound). Signals over `!Send` types
  (`LocalStorage`) can't be mutated from a worker. That matches
  upstream's design intent — `LocalStorage` exists precisely to
  opt out of `Send`.
- **Reading from a worker is still inadvisable.** Technically
  sound (RwLock handles concurrent reads), but you bypass the
  reactive-tracking invariants and get unsubscribed snapshots.
  No framework support for it; if you need a worker-side read,
  shadow the value in worker-local state.
- The `[track_caller]` debug-mode warning that fires when
  `set` silently no-ops (see `traits.rs:550-560`) might mention
  read-guard contention rather than disposal. Workers using
  `try_set` get an explicit `Some(v)` return without the
  warning, so this is only relevant if user code uses `.set`
  blindly on a disposed signal.

The rest of this file is the original design-space analysis,
kept for historical context and in case we want to revisit
LocalStorage / push-API design later.

---

## The problem

Native ports have a hard main-thread constraint: AppKit, UIKit, and
GTK all require UI mutations on the run-loop thread. The
framework's reactive spawner runs on main and polls all reactive
futures there. Async I/O runtimes (tokio, compio, …) own their own
threads; results must come back to main to update signals safely.

The Working with Async docs currently document four patterns. Pattern
4 (push from tokio → main via `on_main`) needs the tokio task to
poke a `!Send` `RwSignal` from a `Send` closure. The naive
`SendWrapper<RwSignal>` approach breaks under repeated use because
`SendWrapper`'s `Clone` and `Drop` deref the inner on whatever
thread is calling, and panic if it isn't the origin thread. The
example currently works around this with a `thread_local!` anchor —
ugly, but correct.

Whatever we build for cross-thread signal updates is in tension with
several upstream design choices in `reactive_graph`, and rushing it
risks painting ourselves into a corner. Hence the shelf.

## The pieces, accurately

### What `RwSignal<T>` actually is

`RwSignal<T, S = SyncStorage>` is `ArenaItem<ArcRwSignal<T>, S>` —
a `Copy` `NodeId` into the current `Owner`'s arena. The arena holds
the `ArcRwSignal` clone. When the Owner disposes (component unmount,
navigation, etc.), `arena.remove(node_id)` runs and the arena's
clone of `ArcRwSignal` drops.

The arena is per-process (one global slotmap) but entries are owned
by Owners (which form a tree following the component tree). An
arena entry can outlive any given thread only if the data inside it
is `Send + Sync` — the `SyncStorage` impl `Storage<T>` requires
`T: Send + Sync + 'static`. So actually, `RwSignal<T, SyncStorage>`
itself *is* `Send + Sync` when `T: Send + Sync` — the only thing
keeping it from being trivially safe across threads is that the
notification semantics weren't designed with that in mind.

`LocalStorage` is the variant for `!Send` `T`s; it uses a
`SendWrapper<T>` inside the arena and the access methods (`try_with`
etc.) will panic if called from a thread other than the one that
stored the value. `RwSignal<T, LocalStorage>` is the right shape
when `T: !Send`.

### What `ArcRwSignal<T>` is

```rust
pub struct ArcRwSignal<T> {
    pub(crate) value: Arc<RwLock<T>>,
    pub(crate) inner: Arc<RwLock<SubscriberSet>>,
}
```

The data lives in `Arc<RwLock<T>>` — owned directly by every
`ArcRwSignal` clone. **No arena involvement.** When the Owner
disposes the arena's clone, anyone still holding an
`ArcRwSignal<T>` keeps the storage alive. `Send + Sync` when
`T: Send + Sync`, `Clone` is `Arc::clone`.

There are `From` conversions both ways. `RwSignal::from(arc_signal)`
registers a new arena entry; `ArcRwSignal::from(rw_signal)` extracts
the arena's `ArcRwSignal` via `try_get_value`, panics if the slot
is gone. So you can hold *both* simultaneously: a `RwSignal` for
ergonomic Copy use in `view!{}`, plus an `ArcRwSignal` you've
cloned out for cross-thread mutation. They share the underlying
`Arc<RwLock<T>>` — mutating either notifies subscribers on both.

### How `set` propagates

`ArcRwSignal::set(v)` (via the `Set` trait, built on `Write` →
`try_write`):

1. Acquire the value `RwLock` for write.
2. Replace the value.
3. Walk the `SubscriberSet` calling `Notify::notify` on each.
4. Each subscriber's `notify` typically marks dirty + wakes a
   `Waker`; the actual re-run happens later when the spawner polls
   the woken future.

Steps 1–3 happen on the calling thread. Whether step 3 is *safe*
to do off-main depends on whether any concrete `Notify` impl
reaches into thread-local state (`OWNER`, `EFFECT`, the per-thread
arena, the spawner's task queue). We did not audit every `Notify`
impl in `reactive_graph` and that audit is part of the deferred
work.

`RwSignal::set` is the same thing wrapped in
`inner.try_with_value(|n| n.try_write_untracked())`, which gates on
the arena entry still existing — if the Owner disposed,
`try_with_value` returns `None` and the set is a **silent no-op**.
Not a panic, not an error. (This is the failure mode that ruled out
the naive `MainSignal<RwSignal<T>>` wrapper in our earlier design.)

### What upstream Leptos says

> The values that are stored in signals must be `Send + Sync`. This
> is because the reactive system actually supports multi-threading:
> signals can be sent across threads, and the whole reactive graph
> can work across multiple threads.

So upstream considers cross-thread signal mutation a first-class
use case (motivated by Axum server-side rendering on Tokio's
multi-thread executor). The Send-ness of the data is enforced; the
Send-ness of the surrounding reactive graph operations is
presumably also intended. We need to verify this claim by reading
notification code, not just take it at face value.

`Signal<T, LocalStorage>` (and the rest of the `LocalStorage`
variants) exist precisely to opt *out* of Send when `T` isn't.

## Design directions we considered

### A. `MainSignal<RwSignal<T>>` — wrap the Copy token

Mark `Send + Sync` via `unsafe impl`. The set/update methods would
`on_main(move || sig.set(v))`. Compiles. Wrong because:

- `RwSignal::set` silently drops updates after Owner disposal, so
  the tokio side has no way to know its work landed.
- Pretends `RwSignal` is thread-safe when its arena lookup is
  thread-conditional (the `try_with_value` succeeds on any thread,
  but only because the `SyncStorage` arena is global; the user's
  `LocalStorage` signals would actually panic).

### B. `RemoteSignal<T>` wrapping `ArcRwSignal<T>`

The good shape, modulo unresolved questions:

```rust
#[derive(Clone)]
pub struct RemoteSignal<T: Send + Sync + 'static> {
    inner: ArcRwSignal<T>,
}

impl<T: Send + Sync + 'static> RemoteSignal<T> {
    pub fn new(v: T) -> Self { Self { inner: ArcRwSignal::new(v) } }
    pub fn as_signal(&self) -> RwSignal<T> { self.inner.clone().into() }
    pub fn set(&self, v: T) {
        let s = self.inner.clone();
        on_main(move || s.set(v));
    }
    pub fn update<F: FnOnce(&mut T) + Send + 'static>(&self, f: F) {
        let s = self.inner.clone();
        on_main(move || s.update(f));
    }
}
```

- Storage independent of Owner lifecycle.
- `Send + Sync + Clone` for free.
- No `unsafe`.
- View code uses `count.as_signal().get()` which is a normal
  `RwSignal`.

Open questions before shipping:

1. **Do we actually need `on_main` around the `set`?** If
   `ArcRwSignal::set` is genuinely thread-safe (upstream claims it
   is for the Axum case), we could let the tokio side mutate
   directly and only marshal back when *subscriber side-effects*
   touch the UI. Subscriber re-runs already go through the
   spawner, which is main-thread. So the question is: do any
   `Notify` impls do work that must run on the notifier's thread?
   Needs an audit.

2. **What about `T: !Send`?** `LocalStorage` signals can't be put
   inside `ArcRwSignal` (which requires `T: Send + Sync` for the
   `Arc<RwLock<T>>`). The user would just not have access to
   `RemoteSignal` for those. That's OK — it matches the
   `Send + Sync` requirement upstream documents.

3. **Naming.** `RemoteSignal`, `MainSignal`, `SendableSignal`,
   `ArcSignal` (already taken by upstream for read-only Arc-backed
   read signals). Bikeshed.

4. **Read-side from off-main.** `RemoteSignal::get()` would
   require `T: Clone` (you'd be reading a snapshot off the
   `Arc<RwLock<T>>`). Doable, but mostly useful in the tokio →
   tokio direction; main code uses `as_signal().get()`. Probably
   ship without it and add later.

### C. Marshal-via-dispatcher abstraction in `common/leptos`

Independent of A/B. Add an opt-in callback registration:

```rust
// common/leptos
pub fn set_main_dispatcher(f: fn(Box<dyn FnOnce() + Send>));
pub fn on_main(f: impl FnOnce() + Send + 'static);
```

Cocoa port wires
`set_main_dispatcher(|f| DispatchQueue::main().exec_async(f))`,
GTK wires `MainContext::default().invoke(...)`, iOS reuses cocoa's.
`RemoteSignal` (if we ship it) uses `leptos::on_main` rather than
`apple_shared::on_main`, so it works on all ports.

Decision: yes, eventually. Not on the critical path right now —
cocoa-side `on_main` in `apple_shared` covers cocoa + iOS, GTK
will need its own helper. Promotion to `common/leptos` is a refactor
when the third port lands.

### D. Skip wrapper entirely, document `ArcRwSignal` directly

The minimal-API option: tell users "for cross-thread signal
updates, hold an `ArcRwSignal<T>` and call `.set()` from within
`on_main(...)`." No new types. Cost: every user re-derives the
recipe. Benefit: zero framework surface.

### E. Marshal subscriber notifications instead of mutations

Inversion: allow `ArcRwSignal::set` from anywhere, but make the
notification step itself marshal back to main. Would require
patching `reactive_graph` (or wrapping `Notify`) to detect main vs
off-main and dispatch accordingly. High blast radius, touches the
core graph. Probably not.

## What blocks shipping

In rough order:

1. **`Notify` thread-safety audit.** Walk every `impl Notify for …`
   in `reactive_graph` and classify: thread-safe, thread-conditional
   (uses thread-local), or thread-hostile (panics on cross-thread).
   This is the single piece of information that decides whether we
   need `on_main` wrapping or not.

2. **Confirm upstream's "reactive graph supports multi-threading"
   claim against the actual notification code.** It might be true
   for *server* contexts where every spawn happens on the same
   tokio runtime and the graph never crosses runtimes; but for
   *GUI* where there's a hard main thread + an unrelated runtime,
   the semantics may be subtly different.

3. **`LocalStorage` story.** Decide whether `RemoteSignal` makes
   sense only for `Send + Sync` `T`s, or whether there's a
   `LocalRemoteSignal` worth building (probably not — if `T` is
   `!Send` you can't ship its updates from a worker anyway).

4. **Marshaling abstraction (C above).** Pull `on_main` into
   `common/leptos` so the API works on GTK.

5. **Naming and API surface.** Settle on `RemoteSignal` vs other
   names. Decide whether to expose `set`, `update`, both, plus
   write-only / read-only variants.

## The current workaround

`docs/book/src/async/README.md` documents two shapes for pattern 4:

- **One-shot push**: move a `SendWrapper<RwSignal<T>>` into a single
  `on_main` closure. Safe because there's no clone-on-worker and
  the wrapper is consumed exactly once on main.
- **Repeated push**: anchor the signal in a `thread_local!` on main,
  have the worker call `on_main(|| THREAD_LOCAL.with(...))`. The
  closure captures the `thread_local!` static (Send) rather than
  the signal handle.

`cocoa/examples/async_patterns` uses the `thread_local!` variant.
Both shapes are honest; neither is pretty. When the design above
lands, the example collapses to ~3 lines and the book chapter
loses ~30 lines of caveats.

## Files involved

When we re-open this:

- `common/reactive_graph/src/signal/{rw,arc_rw}.rs` — RwSignal and
  ArcRwSignal definitions, the `From` conversions.
- `common/reactive_graph/src/owner/{arena_item,storage,arena}.rs` —
  arena lifecycle and the SyncStorage/LocalStorage split.
- `common/reactive_graph/src/traits/{notify,set,write}.rs` —
  notification path; the audit target.
- `common/reactive_graph/src/effect/*.rs` — the most common
  `Notify` consumers; check their thread-safety.
- `apple_shared/src/main_thread.rs` — current home of `on_main`.
- `docs/book/src/async/README.md` — pattern 4 docs to rewrite.
- `cocoa/examples/async_patterns/src/main.rs` — `TickStream`
  section to rewrite.
