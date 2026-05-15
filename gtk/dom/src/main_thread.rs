//! Hop work back to the GTK main thread's run loop.
//!
//! Counterpart to `apple_shared::on_main` (which uses libdispatch on
//! macOS/iOS). On GTK we use `glib::idle_add_once`, which attaches a
//! one-shot idle source to the default `MainContext` — the same
//! main context `Application::run` iterates. The closure runs at
//! idle priority on the GTK main thread on the next main-loop tick.
//!
//! Always defers, even when called from the main thread itself
//! (matches libdispatch's `exec_async` semantics — `on_main` is the
//! portable "send work to main" primitive across all native ports).
//!
//! # Example
//!
//! ```ignore
//! use gtk_dom::on_main;
//!
//! std::thread::spawn(|| {
//!     // … off-main work …
//!     on_main(move || {
//!         // back on the GTK main thread; safe to update signals
//!         // and call gtk widgets.
//!     });
//! });
//! ```

/// Schedule `f` to run on the GTK main thread's run loop.
///
/// Returns immediately. `f` runs later, on the next main-loop tick.
/// Safe to call from any thread.
pub fn on_main<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    glib::idle_add_once(f);
}
