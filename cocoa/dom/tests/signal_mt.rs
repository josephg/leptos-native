//! Cross-thread `reactive_graph` invariants the async docs rely on.
//!
//! Validates the audit conclusions captured in `SIGNAL_MT.md`:
//! signals can be written from a worker thread, the notification
//! cascade only flips atomic flags + wakes wakers (no UI touches),
//! and the effect body re-runs on the AppKit main thread via the
//! framework's `cocoa_dom::spawner` (libdispatch).
//!
//! Also covers the disposal handshake the worker-shutdown pattern
//! depends on (`try_set` return value, `on_cleanup` running before
//! arena disposal).

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::spawner;
use reactive_graph::{
    effect::RenderEffect,
    owner::{on_cleanup, Owner},
    prelude::*,
    signal::{ArcRwSignal, RwSignal},
};
use std::{
    mem,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    },
};

// ---------------------------------------------------------------------
// (1) ArcRwSignal::set from a worker thread → main-thread Effect fires.
// ---------------------------------------------------------------------

fn arc_rw_set_from_worker_fires_main_effect() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();

    let sig = ArcRwSignal::new(0i32);
    let observed = Arc::new(Mutex::new(Vec::<i32>::new()));

    // Effect re-runs on signal change. The body runs on whatever
    // thread the spawner polls it from — our AppKit spawner means
    // main. We `mem::forget` so the effect lives past this scope.
    let eff = {
        let sig = sig.clone();
        let observed = observed.clone();
        RenderEffect::new(move |_| {
            let v = sig.get();
            observed.lock().unwrap().push(v);
        })
    };
    mem::forget(eff);

    // First poll: drains the initial run (which sees 0).
    common::pump_run_loop(0.2);
    assert_eq!(observed.lock().unwrap().as_slice(), &[0]);

    // Worker writes. The notify cascade fires on the worker; the
    // effect re-poll is dispatched to main via libdispatch.
    let sig_w = sig.clone();
    std::thread::spawn(move || sig_w.set(42))
        .join()
        .expect("worker joined");

    common::pump_run_loop(0.2);

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        &[0, 42],
        "main-thread effect should have seen the worker's write"
    );
}

// ---------------------------------------------------------------------
// (2) RwSignal::set (arena-routed) from a worker thread fires.
// ---------------------------------------------------------------------

fn rw_set_from_worker_fires_main_effect() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();

    let sig = RwSignal::new(0i32);
    let observed = Arc::new(Mutex::new(Vec::<i32>::new()));

    {
        let observed = observed.clone();
        mem::forget(RenderEffect::new(move |_| {
            observed.lock().unwrap().push(sig.get());
        }));
    }
    common::pump_run_loop(0.2);

    // Worker writes through the arena-routed Copy token.
    std::thread::spawn(move || sig.set(7))
        .join()
        .expect("worker joined");
    common::pump_run_loop(0.2);

    assert_eq!(observed.lock().unwrap().as_slice(), &[0, 7]);
}

// ---------------------------------------------------------------------
// (3) Concurrent writers: two workers race; the main-thread effect
//     ends up seeing both values, doesn't deadlock or panic.
// ---------------------------------------------------------------------

fn concurrent_writers_dont_deadlock() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();

    let sig = ArcRwSignal::new(0u32);
    let seen = Arc::new(AtomicU32::new(0));
    {
        let sig = sig.clone();
        let seen = seen.clone();
        mem::forget(RenderEffect::new(move |_| {
            // Tally distinct values by ORing them; collision-free
            // for a small set of unique writes.
            seen.fetch_or(sig.get(), Ordering::SeqCst);
        }));
    }
    common::pump_run_loop(0.1);

    let sig1 = sig.clone();
    let sig2 = sig.clone();
    let t1 = std::thread::spawn(move || {
        for _ in 0..50 {
            sig1.set(0b01);
        }
    });
    let t2 = std::thread::spawn(move || {
        for _ in 0..50 {
            sig2.set(0b10);
        }
    });
    t1.join().unwrap();
    t2.join().unwrap();

    // Both writers' last values may still be sitting in the
    // notification pipeline; pump generously.
    common::pump_run_loop(0.3);

    let mask = seen.load(Ordering::SeqCst);
    assert!(
        mask & 0b11 != 0,
        "effect saw nothing; expected at least one writer's value"
    );
    // We don't assert mask == 0b11 because the racing nature means
    // both writers' last-writes-win can collapse to a single value
    // visible by the time the effect polls. The relevant assertion
    // is "didn't deadlock or panic".
}

// ---------------------------------------------------------------------
// (4) Disposal handshake — try_set returns Some(v) after dispose.
// ---------------------------------------------------------------------

fn try_set_returns_none_then_some_after_dispose() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();

    let sig = RwSignal::new(0i32);

    // Live: returns None.
    assert_eq!(sig.try_set(1), None);
    // Dispose the owner — the arena entry goes away.
    drop(owner);

    // After dispose: returns Some with our value back.
    let returned = sig.try_set(99);
    assert_eq!(
        returned,
        Some(99),
        "try_set on a disposed signal should hand the value back"
    );
}

// ---------------------------------------------------------------------
// (5) Worker uses try_set's Some/None to self-terminate. This is
//     the exact shape recommended by the async docs' Pattern 4.
// ---------------------------------------------------------------------

fn worker_shuts_down_on_dispose() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();
    let sig = RwSignal::new(0u32);

    // Worker writes in a loop and breaks when it sees a Some.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let writes_after_dispose = Arc::new(AtomicU32::new(0));
    let writes_clone = writes_after_dispose.clone();
    let worker = std::thread::spawn(move || {
        let mut n = 0u32;
        loop {
            n += 1;
            if sig.try_set(n).is_some() {
                writes_clone.fetch_add(1, Ordering::SeqCst);
                let _ = tx.send(());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    // Let it tick for a bit.
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Dispose; worker should notice on next try_set and exit.
    drop(owner);

    rx.recv_timeout(std::time::Duration::from_secs(1))
        .expect("worker should have signalled exit");
    worker.join().expect("worker joined cleanly");

    assert_eq!(
        writes_after_dispose.load(Ordering::SeqCst),
        1,
        "worker should have made exactly one Some(v) observation before exiting"
    );
}

// ---------------------------------------------------------------------
// (6) on_cleanup runs *before* arena disposal — we can still read
//     signal values inside the cleanup closure. This is the
//     load-bearing assumption of the EagerCancel pattern (we read
//     a JoinHandle out of a signal slot inside on_cleanup).
// ---------------------------------------------------------------------

fn on_cleanup_runs_before_arena_disposal() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let owner = Owner::new();
    owner.set();

    let sig = RwSignal::new(123u32);
    let observed_in_cleanup = Arc::new(Mutex::new(None::<u32>));
    {
        let observed = observed_in_cleanup.clone();
        on_cleanup(move || {
            // If arena disposal had already run, this try_get_untracked
            // would return None. We assert it sees the value.
            let v = sig.try_get_untracked();
            *observed.lock().unwrap() = v;
        });
    }

    drop(owner);

    assert_eq!(
        *observed_in_cleanup.lock().unwrap(),
        Some(123),
        "on_cleanup should observe the live signal value before arena disposal"
    );
}

// ---------------------------------------------------------------------
// (7) on_cleanup ordering: cleanups run when their owning Owner
//     disposes, not when an outer Owner disposes. Verifies nested
//     scopes work correctly.
// ---------------------------------------------------------------------

fn on_cleanup_scoped_to_creating_owner() {
    let _mtm = common::test_mtm();
    let _ = spawner::init();
    let outer = Owner::new();
    outer.set();
    let fired = Arc::new(AtomicU32::new(0));

    {
        let inner = Owner::new();
        inner.set();
        let f = fired.clone();
        on_cleanup(move || {
            f.fetch_add(1, Ordering::SeqCst);
        });
        drop(inner);
    }

    assert_eq!(
        fired.load(Ordering::SeqCst),
        1,
        "inner cleanup should fire when inner owner disposes"
    );

    drop(outer);

    assert_eq!(
        fired.load(Ordering::SeqCst),
        1,
        "outer owner disposing should not re-fire the inner cleanup"
    );
}

fn main() {
    common::run_tests(&[
        (
            "arc_rw_set_from_worker_fires_main_effect",
            arc_rw_set_from_worker_fires_main_effect,
        ),
        (
            "rw_set_from_worker_fires_main_effect",
            rw_set_from_worker_fires_main_effect,
        ),
        ("concurrent_writers_dont_deadlock", concurrent_writers_dont_deadlock),
        (
            "try_set_returns_none_then_some_after_dispose",
            try_set_returns_none_then_some_after_dispose,
        ),
        ("worker_shuts_down_on_dispose", worker_shuts_down_on_dispose),
        (
            "on_cleanup_runs_before_arena_disposal",
            on_cleanup_runs_before_arena_disposal,
        ),
        (
            "on_cleanup_scoped_to_creating_owner",
            on_cleanup_scoped_to_creating_owner,
        ),
    ]);
}
