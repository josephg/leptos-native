//! Tests for `gtk_dom::on_main` — GTK wrapper around
//! `glib::idle_add_once`.
//!
//! Counterpart of `cocoa_dom/tests/async_bridge.rs`. Verifies:
//!
//! 1. Closures scheduled from a *background* thread run on the
//!    GTK main thread (the one owning the default `MainContext`).
//! 2. Closures scheduled from the *main* thread itself still
//!    *defer* — they don't run inline. Matches libdispatch's
//!    `exec_async` semantics on the Apple ports.

#![cfg(feature = "gtk")]

mod common;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use leptos_gtk::dom::on_main;

/// Pump the default `MainContext` non-blockingly for up to
/// `timeout_secs`, returning early as soon as the predicate is
/// true. Glib's `iteration(may_block=false)` drains all pending
/// sources without sleeping; we loop with a tiny sleep between
/// passes so cross-thread idle sources have time to arrive.
fn pump_until<F: Fn() -> bool>(predicate: F, timeout_secs: f64) {
    let ctx = glib::MainContext::default();
    let start = std::time::Instant::now();
    while !predicate() && start.elapsed().as_secs_f64() < timeout_secs {
        while ctx.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

// ---------------------------------------------------------------------
// (1) Closure scheduled from a worker runs on the GTK main thread.
// ---------------------------------------------------------------------

fn on_main_from_worker_runs_on_main_thread() {
    let main_thread_id = std::thread::current().id();
    let ran_on_main = Arc::new(AtomicBool::new(false));
    let observed_thread =
        Arc::new(std::sync::Mutex::new(None::<std::thread::ThreadId>));

    let flag = ran_on_main.clone();
    let observed = observed_thread.clone();
    std::thread::spawn(move || {
        on_main(move || {
            *observed.lock().unwrap() = Some(std::thread::current().id());
            flag.store(true, Ordering::SeqCst);
        });
    })
    .join()
    .expect("worker joined");

    pump_until(|| ran_on_main.load(Ordering::SeqCst), 1.0);

    assert!(
        ran_on_main.load(Ordering::SeqCst),
        "closure never ran (timed out pumping the main context)"
    );
    assert_eq!(
        *observed_thread.lock().unwrap(),
        Some(main_thread_id),
        "closure ran on a different thread than the test runner's main"
    );
}

// ---------------------------------------------------------------------
// (2) Closure scheduled from main still defers — idle_add_once
//     never runs inline.
// ---------------------------------------------------------------------

fn on_main_from_main_defers() {
    let fired = Arc::new(AtomicBool::new(false));
    let fired_for_closure = fired.clone();

    on_main(move || fired_for_closure.store(true, Ordering::SeqCst));

    assert!(
        !fired.load(Ordering::SeqCst),
        "on_main(...) ran inline; expected deferred idle source"
    );

    pump_until(|| fired.load(Ordering::SeqCst), 1.0);
    assert!(
        fired.load(Ordering::SeqCst),
        "on_main(...) should have fired after pumping the main context"
    );
}

// ---------------------------------------------------------------------
// (3) Many cross-thread on_main calls all dispatch — no idle source
//     gets lost.
// ---------------------------------------------------------------------

fn many_on_main_calls_all_fire() {
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

    pump_until(|| count.load(Ordering::SeqCst) == 10, 1.0);

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
