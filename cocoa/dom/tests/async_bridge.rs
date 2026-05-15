//! Tests for `leptos_apple_shared::on_main` (cocoa wrapper around
//! `dispatch2::DispatchQueue::main().exec_async`).
//!
//! Verifies the two contract points documented in
//! `apple_shared/src/main_thread.rs`:
//!
//! 1. Closures scheduled from a *background* thread run on the
//!    AppKit main thread.
//! 2. Closures scheduled from the *main* thread itself still
//!    *defer* — they don't run inline. This matches libdispatch's
//!    `exec_async` semantics and the cross-port `on_main` contract
//!    (GTK's `idle_add_once` behaves the same way).

#![cfg(target_os = "macos")]

mod common;

use leptos_apple_shared::on_main;
use objc2::MainThreadMarker;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

// ---------------------------------------------------------------------
// (1) Closure scheduled from a worker runs on main.
// ---------------------------------------------------------------------

fn on_main_from_worker_runs_on_main_thread() {
    let _mtm = common::test_mtm();
    let ran_on_main = Arc::new(AtomicBool::new(false));
    let ran = ran_on_main.clone();

    std::thread::spawn(move || {
        on_main(move || {
            // If we're not on main, `MainThreadMarker::new()` is None;
            // the assert below would store false. Storing true here is
            // the positive evidence.
            let is_main = MainThreadMarker::new().is_some();
            ran.store(is_main, Ordering::SeqCst);
        });
    })
    .join()
    .expect("worker joined");

    // Drain the main queue. 1 s is generous; the dispatch typically
    // fires in microseconds.
    common::pump_run_loop(1.0);

    assert!(
        ran_on_main.load(Ordering::SeqCst),
        "closure should have run on the main thread"
    );
}

// ---------------------------------------------------------------------
// (2) Closure scheduled from main still defers.
//
// If `on_main` ran inline when called from main, this assertion
// would fail: the flag would be true *before* we pump the run loop.
// ---------------------------------------------------------------------

fn on_main_from_main_defers() {
    let _mtm = common::test_mtm();
    let fired = Arc::new(AtomicBool::new(false));
    let fired_for_closure = fired.clone();

    on_main(move || fired_for_closure.store(true, Ordering::SeqCst));

    // Must not have run yet — `exec_async` defers even when called
    // on the queue's owning thread.
    assert!(
        !fired.load(Ordering::SeqCst),
        "on_main(...) ran inline; expected deferred dispatch"
    );

    common::pump_run_loop(1.0);

    assert!(
        fired.load(Ordering::SeqCst),
        "on_main(...) should have fired after pumping the run loop"
    );
}

// ---------------------------------------------------------------------
// (3) Multiple closures from different threads all dispatch.
// ---------------------------------------------------------------------

fn many_on_main_calls_all_fire() {
    let _mtm = common::test_mtm();
    let count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let c = count.clone();
        handles.push(std::thread::spawn(move || {
            on_main(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    common::pump_run_loop(1.0);

    assert_eq!(
        count.load(Ordering::SeqCst),
        10,
        "all 10 closures should have run on main"
    );
}

fn main() {
    common::run_tests(&[
        (
            "on_main_from_worker_runs_on_main_thread",
            on_main_from_worker_runs_on_main_thread,
        ),
        ("on_main_from_main_defers", on_main_from_main_defers),
        ("many_on_main_calls_all_fire", many_on_main_calls_all_fire),
    ]);
}
