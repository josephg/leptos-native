# Test plan — Async runtime integration

Tests we need for the tokio / compio / signal-thread-safety work
landed 2026-05-15. Cross-cutting: covers cocoa, iOS, and GTK
ports because the patterns are deliberately identical.

Items are `□` (not yet covered) by default. Group ordering is rough
priority — the unit tests at the top are cheap and catch the most
breakage; integration / E2E sit lower because they cost a run loop.

## How to read this

- **Tier** — cheapest test that would catch regressions:
  - *unit* — pure Rust, no run loop, no window.
  - *signal* — uses `reactive_graph` primitives; needs an Owner but
    no UI.
  - *integration* — needs a real port run loop; usually one of the
    existing test harnesses (`cocoa/dom/tests/*`, GTK `gtester`,
    iOS XCUIAutomation).
- **Port** — which port(s) the test should run on. Most of the
  signal-thread-safety tests run against `reactive_graph` directly
  and need no port — `*` covers all.

## 0. Foundation: `Notify` thread-safety audit, codified

The whole story rests on `Notify`-side primitives being thread-safe.
We audited this once (see SIGNAL_MT.md + implementation_log.md);
the next refactor of `reactive_graph` could quietly break the
invariant if no test holds it down.

- ■ **tier:signal**, port:cocoa — Worker thread calls
  `ArcRwSignal::set(v)`; subscriber on main fires.
  (`cocoa/dom/tests/signal_mt.rs::arc_rw_set_from_worker_fires_main_effect`)
- ■ **tier:signal**, port:cocoa — Worker thread calls
  `RwSignal::set(v)` (arena-routed); subscriber on main fires.
  (`signal_mt.rs::rw_set_from_worker_fires_main_effect`)
- ■ **tier:signal**, port:cocoa — Concurrent writers (two worker
  threads writing to the same `ArcRwSignal`) don't deadlock and
  the effect tally proves both values landed.
  (`signal_mt.rs::concurrent_writers_dont_deadlock`)
- □ port:gtk, port:iOS — same three. Mechanical mirror; gtk
  needs a display for the spawner / main-thread test (already
  the case for the existing gtk_dom tests; CI-only).
- □ **tier:signal**, port:`*` — Read on main thread while a
  worker is concurrently writing: read either sees old or new
  value, never garbage. (RwLock semantics; verifies our claim.)
- □ **tier:signal**, port:`*` — Cascade depth: signal A subscribed
  by memo B subscribed by effect C; A.set on worker triggers C
  on main, not anywhere else.

## 1. Disposal handshake

The `try_set` → `Option<T>` contract is the worker-shutdown
primitive. If upstream changes the return type or semantics, our
examples and recommended patterns silently break.

- ■ **tier:signal**, port:cocoa — `RwSignal::try_set` returns
  `None` while alive, `Some(v)` after `Owner::drop`.
  (`signal_mt.rs::try_set_returns_none_then_some_after_dispose`)
- ■ **tier:signal**, port:cocoa — Worker uses `try_set`'s
  Some/None return to self-terminate; exits exactly once on the
  first observation after disposal.
  (`signal_mt.rs::worker_shuts_down_on_dispose`)
- ■ **tier:signal**, port:cocoa — `on_cleanup` runs *before*
  arena disposal of the same Owner.
  (`signal_mt.rs::on_cleanup_runs_before_arena_disposal`)
- ■ **tier:signal**, port:cocoa — `on_cleanup` is scoped to the
  creating Owner, not an outer one.
  (`signal_mt.rs::on_cleanup_scoped_to_creating_owner`)
- □ **tier:signal**, port:`*` — Disposal mid-cascade: signal is
  disposed between `try_set` enqueueing the notify and the
  spawner polling the effect — effect is silently skipped, no
  panic, no leak. (Stronger than the existing tests; would need
  a deliberate race.)

## 2. `apple_shared::on_main` / `gtk_dom::on_main`

- ■ **tier:unit**, port:cocoa — `on_main` from background thread
  schedules a closure that runs on the AppKit main thread.
  (`cocoa/dom/tests/async_bridge.rs::on_main_from_worker_runs_on_main_thread`)
- ■ **tier:unit**, port:cocoa — `on_main` from the main thread
  still defers.
  (`async_bridge.rs::on_main_from_main_defers`)
- ■ **tier:unit**, port:cocoa — Many concurrent `on_main` calls
  from multiple workers all dispatch.
  (`async_bridge.rs::many_on_main_calls_all_fire`)
- ■ **tier:unit**, port:gtk — `on_main` (= `glib::idle_add_once`)
  from background thread runs in the default `MainContext`.
  (`gtk/dom/tests/async_bridge.rs::on_main_from_worker_runs_on_main_thread`)
- ■ **tier:unit**, port:gtk — `on_main` from main thread still
  defers (`idle_add_once` always enqueues).
  (`gtk async_bridge.rs::on_main_from_main_defers`)
- ■ **tier:unit**, port:gtk — Many concurrent `on_main` calls
  all dispatch.
  (`gtk async_bridge.rs::many_on_main_calls_all_fire`)
- □ **tier:unit**, port:iOS — same three. UIKit `on_main` is
  `apple_shared`'s function unchanged; relies on iOS having an
  equivalent of cocoa_dom's test harness (currently doesn't —
  uikit/dom/tests/ is minimal).

## 3. `<image_view bytes=…>`

- ■ **tier:unit**, port:cocoa — `image_view` tag really creates
  an NSImageView.
  (`cocoa/dom/tests/image_bytes.rs::image_view_tag_creates_nsimageview`)
- ■ **tier:unit**, port:cocoa — Valid PNG bytes set the image.
  (`image_bytes.rs::valid_png_bytes_set_image`)
- ■ **tier:unit**, port:cocoa — `Some(non-empty)` → `None`
  clears.
  (`image_bytes.rs::none_clears_image`)
- ■ **tier:unit**, port:cocoa — `Some(&[])` clears (matches
  `set_image_view_path("")`).
  (`image_bytes.rs::empty_slice_clears_image`)
- ■ **tier:unit**, port:cocoa — Garbage bytes don't panic; view
  ends imageless.
  (`image_bytes.rs::garbage_bytes_dont_panic`)
- ■ **tier:unit**, port:cocoa — Replacing valid bytes with new
  valid bytes installs a fresh NSImage object.
  (`image_bytes.rs::replace_with_new_valid_bytes`)
- □ **tier:unit**, port:iOS — Same six cases for
  `UIImage::imageWithData:`.
- □ **tier:unit**, port:cocoa — Reactive `bytes()` builder fires
  on signal change end-to-end (Render path, not just the
  low-level setter). Best covered once we have a leptos_cocoa
  test harness — bigger lift.

## 4. Pattern 1 — `tokio::spawn(io).await` via `AsyncDerived`

- □ **tier:integration**, port:cocoa — `ipify`-style flow with a
  mock HTTP endpoint (no network): construct `AsyncDerived`,
  poll until resolved, assert the produced `Some(Ok(value))`.
  Mock the HTTP via `httpmock` or a local TCP listener.
- □ **tier:integration**, port:cocoa — Re-trigger by changing an
  input signal: assert `AsyncDerived` returns `None` during
  re-fetch, then `Some(...)` again.
- □ **tier:integration**, port:cocoa — `Builder::new_current_thread`
  parked-on-side-thread variant produces the same result.

## 5. Pattern 2 — oneshot + drop-to-cancel

- □ **tier:integration**, port:cocoa — `CancellableFetch`-style
  flow: `tokio::spawn` + oneshot; assert that dropping the
  receiver causes the sender's `tx.send()` to return Err.
- □ **tier:integration**, port:cocoa — Two consecutive Start
  clicks: first task's receiver is dropped, second task's
  result is what shows up.

## 6. Pattern 3 — persistent mpsc worker

- □ **tier:integration**, port:cocoa — `MathService`-style: send
  N requests in flight, all resolve via their per-request
  oneshots, ordering is FIFO on the worker.
- □ **tier:integration**, port:cocoa — Dropping the `mpsc::Sender`
  causes the worker to exit (recv returns None).

## 7. Pattern 4 — lazy disposal-driven shutdown (TickStream)

This is the pattern we'd most regret breaking — it's the
recommended default for tick-style work.

- □ **tier:signal**, port:`*` — `tokio::spawn`'d worker calling
  `signal.try_set(n)` exits on the first call after the signal's
  Owner has disposed. Verify by joining the spawn handle.
- □ **tier:integration**, port:cocoa — Mount `TickStream` under
  `<Show>`, flip `when` to false, then to true; assert two
  distinct worker instances ran (via the instance counter), and
  the first worker's `JoinHandle` resolved.
- □ **tier:integration**, port:cocoa — Counter resets to zero on
  remount (verifies the fresh signal).
- □ **tier:integration**, port:cocoa — Hide indefinitely → no
  worker leak. Use a `Weak<RwLock<TaskCount>>`-style probe or
  Drop instrumentation on captured state.
- □ **tier:integration**, port:iOS — Same as cocoa above.
- □ **tier:integration**, port:gtk — Same.

## 8. Pattern 5 — eager cancellation via `on_cleanup` + `abort`

- □ **tier:signal**, port:`*` — Long sleep (10 s) `tokio::spawn`'d,
  `JoinHandle` aborted immediately; the sleep wakes with
  cancellation within `<100ms`. Verifies `abort()` semantics.
- □ **tier:integration**, port:cocoa — Mount `EagerCancel`,
  click Start, immediately unmount via `<Show>`. Assert the
  task's `on_cleanup` ran (Drop-tracked captured state should
  release) within one main-loop tick.
- □ **tier:integration**, port:cocoa — Click Start twice rapidly:
  first task is aborted by the second's start handler; only the
  second's result shows up.
- □ **tier:integration**, port:cocoa — Cancel button works while
  the task is in flight; status flips to "cancelled" and the
  task's body doesn't complete.
- □ **tier:integration**, port:iOS+gtk — Same as cocoa above.

## 9. Compio dispatcher

- □ **tier:integration**, port:cocoa — `Dispatcher::new()` +
  `dispatch(|| async { … })` round-trip: worker computes a value
  and the main-thread future resolves to it.
- □ **tier:integration**, port:cocoa — Dispatcher drop joins all
  workers cleanly (no leaked threads).

## 10. Cross-port consistency

These are duplicate-of-each-other checks that catch divergence.

- □ **tier:integration** — Run `async_patterns` on cocoa, iOS,
  GTK; assert the four (now five) pattern sections produce the
  same observable behaviour for a scripted input sequence.
- □ **tier:unit** — Each port re-exports `on_main` with the same
  signature `fn on_main<F: FnOnce() + Send + 'static>(f: F)`.

## 11. Cancellation interaction with multiple patterns

Edge cases that aren't covered by any single pattern's tests.

- □ **tier:integration**, port:cocoa — `EagerCancel` task that
  internally calls `signal.try_set(v)`: aborting via
  `JoinHandle::abort` between an `.await` and the `try_set` call
  doesn't leak (drop ordering of captures).
- □ **tier:integration**, port:cocoa — Component containing both
  `TickStream` and `EagerCancel` (the example case): unmount
  cleanly stops both within one main-loop tick after the next
  tick check.

## 12. Documentation / contract tests

- □ **tier:unit** — `mdbook test` (or rustdoc-style code-block
  compile checks) on `docs/book/src/async/*.md`. Currently the
  snippets are by-eye only.
- □ **tier:unit** — `cargo doc --no-deps -p apple_shared
  -p gtk_dom` builds cleanly with no broken intra-doc links to
  `on_main` / `RwSignal::try_set` / `on_cleanup`.

## 13. Smoke tests / examples-launch

These already exist informally (the "did the example run for 5
seconds without panicking" check from the conversation). Codify:

- □ **tier:integration**, port:cocoa — Each of `ipify`,
  `placecats`, `ipify_current_thread`, `ipify_compio`,
  `async_patterns` launches and survives for 3 s under
  `XCUIAutomation` (or the lighter weight in-process harness
  once it exists).
- □ **tier:integration**, port:iOS — `ipify`, `async_patterns`
  via `run_ios.sh -t 5`.
- □ **tier:integration**, port:gtk — `ipify`, `async_patterns`
  via a `cargo run -p ipify_gtk` smoke runner on a Linux CI
  machine. (Not testable on the macOS host without gtk4.)

## What we deliberately don't test

- Network reachability — `ipify.org` / `placecats.com` being up
  is not our problem. The HTTP-using tests above use mock
  servers or local TCP listeners.
- libdispatch / glib internals — assume they work; we test only
  the contract surface (`on_main` defers; closures run on main).
- Upstream `reactive_graph` internals beyond the `Notify`
  contract surface — we test what we depend on, not what they
  guarantee.

## Test infrastructure work that would help

Roughly in dependency order:

- A **`SpawnTracker`** primitive: `let (probe, handle) =
  SpawnTracker::new()`; the probe is a `Send + Sync` token that
  becomes `is_dropped()` when its associated state has dropped.
  Lets disposal-driven tests assert "task captures got freed"
  without polling timestamps.
- A **mock HTTP listener** that binds an ephemeral port, takes
  scripted responses, and exposes a base URL for tests. Reusable
  across the Pattern 1 / 2 / 5 tests.
- An **in-process XCUIAutomation alternative** for cocoa — spin
  up `NSApplication.run` on a worker thread, post events,
  observe. Currently deferred (see tests_macos.md §0); landing
  it unlocks most of the Pattern 4 / 5 integration tests.
- A **shared test harness** that drives `cargo run`'ing each
  example for N seconds + asserts the process exits with 0
  (currently 124 from `timeout`). Mostly bash / xtask.
