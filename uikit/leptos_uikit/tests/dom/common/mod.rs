//! Shared test helpers for `leptos_uikit` integration tests.
//!
//! UIKit demands the main thread for almost every API. Cargo's
//! default test harness can spawn a worker thread per test, which
//! makes `MainThreadMarker::new()` return `None`. Each test file
//! uses `harness = false` (configured in `leptos_uikit/Cargo.toml`)
//! and calls [`run_tests`] from its `fn main()` to run the test
//! bodies sequentially on the actual main thread.

#![cfg(target_os = "ios")]
#![allow(dead_code)]

use leptos_uikit::dom::MainThreadMarker;

/// `MainThreadMarker` for the test runner's main thread. Panics if
/// the test isn't running on the main thread.
pub fn test_mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect(
        "UIKit tests must run on the main thread. iOS-sim cargo test \
         already does so; if this panics, a test moved threads or \
         the runner forked.",
    )
}

/// Custom test runner: runs each `fn()` on the current (main)
/// thread, catches panics, prints a libtest-style summary, and
/// exits with code 1 on any failure. Same shape as the cocoa_dom
/// helper — kept independent so the iOS test crate doesn't need
/// to depend on the cocoa one.
pub fn run_tests(tests: &[(&'static str, fn())]) {
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
