//! Hop work back to the main thread's run loop.
//!
//! On Cocoa and UIKit the main thread owns the run loop and is the
//! only thread allowed to touch NSView/UIView or mutate the reactive
//! graph our spawner drives. Background threads (a tokio worker, a
//! database connection thread, a custom `std::thread::spawn`) that
//! need to update signals or call UI code must marshal the work
//! back to main first.
//!
//! [`on_main`] is the smallest possible wrapper around libdispatch's
//! main queue to do exactly that. It's intentionally tiny — the
//! whole point is a memorable, port-neutral name for the operation
//! so user code doesn't have to learn AppKit/UIKit dispatch
//! vocabulary.
//!
//! # Example
//!
//! ```no_run
//! use leptos_apple_shared::on_main;
//!
//! std::thread::spawn(|| {
//!     // … long-running work off-main …
//!     on_main(move || {
//!         // back on the main thread; safe to update signals / views
//!     });
//! });
//! ```

use dispatch2::DispatchQueue;

/// Schedule `f` to run on the main thread's run loop.
///
/// Returns immediately. `f` runs later, after the current
/// main-thread call stack unwinds (or, if called from main itself,
/// after the next trip through the run loop).
///
/// Safe to call from any thread. Use this to hand work — signal
/// updates, view manipulation — from a background thread back to
/// the UI thread.
pub fn on_main<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    DispatchQueue::main().exec_async(f);
}
