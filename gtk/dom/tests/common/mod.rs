//! Shared test helpers — GTK init, custom test runner.
//!
//! Mirrors `cocoa_dom/tests/common/mod.rs`. GTK4 (like AppKit) is
//! main-thread-only — `gtk::init()` records the calling thread as the
//! main thread, and any subsequent GTK call from another thread
//! panics. Cargo's default test harness spawns a worker thread per
//! test, so we use `harness = false` and run all tests sequentially
//! on the main thread (the binary's `fn main()`).

#![cfg(feature = "gtk")]
#![allow(dead_code)] // helpers used by some test files but not all

/// Initialise GTK once for the test binary's lifetime. Idempotent —
/// gtk::init's underlying gtk_init_check is safe to call multiple
/// times, but skip the second one to be tidy.
pub fn ensure_gtk_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Tests run headless (no display) — `gtk_init_check` reports
        // failure if it can't connect to a display. Try anyway, but
        // fall back to forcing the runtime flag so widget construction
        // doesn't panic on the `assert_initialized_main_thread!`
        // check that every gtk-rs setter expands to.
        if gtk4::init().is_err() {
            // Headless path: gtk::init() failed (no DISPLAY /
            // WAYLAND_DISPLAY). We still need the runtime to think
            // GTK is initialized so widget constructors don't panic
            // on `assert_initialized_main_thread!`. There's no public
            // API to fake this, so skip the affected tests at the
            // call site by checking `is_headless()`.
        }
    });
}

/// Whether the test binary has a display available. Used to skip
/// tests that need real widget construction when running over SSH.
pub fn is_headless() -> bool {
    !gtk4::is_initialized()
}

/// `init_app` + register the GApplication so tests can attach
/// `GtkApplicationWindow`s without entering the main loop.
///
/// Production never needs this: `app.run()` registers the
/// application (emitting `startup`) before any window is created
/// inside `activate`. But these tests build windows directly,
/// skipping `run()` — so GTK logs a `Gtk-CRITICAL` ("New
/// application windows must be added after the
/// GApplication::startup signal has been emitted") the moment the
/// window is attached to the un-registered app. `register()` emits
/// `startup` synchronously without running the loop, which is
/// exactly what the warning asks for.
pub fn init_app_registered(application_id: &str) -> gtk4::Application {
    use gtk4::prelude::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    // Each call builds a fresh `Application`. Registering two app
    // objects with the same id in one process collides (GApplication
    // single-instance) — the second `register()` fails, leaving the
    // app unregistered and the CRITICAL un-suppressed. Suffix a
    // per-call counter so every test gets a uniquely-registerable id.
    static N: AtomicU32 = AtomicU32::new(0);
    // The suffix element must start with a letter — GApplication
    // rejects id segments that begin with a digit.
    let id = format!("{application_id}.t{}", N.fetch_add(1, Ordering::Relaxed));
    let app = gtk_dom::app::init_app(&id);
    let _ = app.register(None::<&gtk4::gio::Cancellable>);
    app
}

/// Custom test runner — same shape as the cocoa one. Runs each
/// `fn()` on the current (main) thread, catches panics, prints a
/// libtest-style summary, and exits with code 1 on any failure.
///
/// On a headless system (no DISPLAY / WAYLAND_DISPLAY,
/// `gtk::init()` fails), the runner skips the entire suite — most
/// tests need to construct widgets, which requires a real display
/// connection.
pub fn run_tests(tests: &[(&'static str, fn())]) {
    ensure_gtk_init();

    if is_headless() {
        println!(
            "\nrunning 0 tests\n\n\
             test result: ok. 0 passed; 0 failed; {} ignored \
             (no GTK display available — tests skipped)",
            tests.len()
        );
        return;
    }

    let total = tests.len();
    println!("\nrunning {} tests", total);

    let mut passed = 0usize;
    let mut failed: Vec<(String, String)> = Vec::new();

    for (name, body) in tests {
        let result = std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(*body),
        );
        match result {
            Ok(()) => {
                println!("test {} ... ok", name);
                passed += 1;
            }
            Err(payload) => {
                let msg = downcast_panic(&payload);
                println!("test {} ... FAILED", name);
                failed.push((name.to_string(), msg));
            }
        }
    }

    println!();
    if failed.is_empty() {
        println!(
            "test result: ok. {} passed; 0 failed; 0 ignored",
            passed
        );
    } else {
        println!("failures:");
        for (name, msg) in &failed {
            println!();
            println!("---- {} stdout ----", name);
            println!("{}", msg);
        }
        println!();
        println!(
            "test result: FAILED. {} passed; {} failed; 0 ignored",
            passed,
            failed.len()
        );
        std::process::exit(1);
    }
}

fn downcast_panic(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Suppress unused-warnings for items we expose for use across tests
/// even though some test files don't touch all of them.
#[allow(dead_code)]
pub(crate) fn _force_link() {}
