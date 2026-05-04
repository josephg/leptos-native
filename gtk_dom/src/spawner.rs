//! GTK main-thread executor for `any_spawner`.
//!
//! Reactive_graph uses `any_spawner::Executor::spawn_local` to drive
//! effects (each Effect spawns a tiny async task that loops on a
//! notification channel and re-runs the effect body).
//!
//! We use `any_spawner`'s built-in `init_glib()`, which delegates to
//! `glib::MainContext::spawn` / `spawn_local`. These create a GSource
//! attached to the default main context — the same one GTK's
//! `Application::run` iterates. The source is dispatched on the next
//! main-loop iteration, and the Rust waker integration (via GLib's
//! `TaskSource`) correctly wakes the future when reactive signals
//! change.
//!
//! Why not `MainContext::invoke`? It runs the closure *inline* when
//! the current thread owns the context (which it does during GTK
//! signal dispatch). `spawn` / `spawn_local` always defer.
//!
//! Why not hand-roll a Wake impl? The built-in GLib adapter does
//! exactly what we need. Earlier debugging mistakenly blamed
//! `JoinHandle::Drop` for detaching sources — `JoinHandle` and
//! `SourceId` are both plain integer wrappers with no destructive
//! Drop in glib 0.21. The actual bug was `reactive_graph/effects`
//! not being enabled in the `native-ui` feature (fixed in
//! leptos/Cargo.toml).
//!
//! # Lifecycle
//!
//! Call [`init`] once, before constructing the first signal/effect.
//! It's idempotent — second and later calls return
//! `Err(ExecutorError::AlreadySet)` and can be ignored.

use any_spawner::{Executor, ExecutorError};

/// Initialise the global GTK main-thread executor. Idempotent.
///
/// Must be called from the GTK main thread.
pub fn init() -> Result<(), ExecutorError> {
    Executor::init_glib()
}
