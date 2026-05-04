//! GTK main-thread executor for `any_spawner`.
//!
//! Reactive_graph uses `any_spawner::Executor::spawn_local` to drive
//! effects (each Effect spawns a tiny async task that loops on a
//! notification channel and re-runs the effect body). Without an
//! executor wired in, signal updates never trigger effect runs and
//! the UI never updates.
//!
//! glib's main context is futures-aware out of the box: we hand it
//! the future via `MainContext::spawn_local`, and glib drives polling
//! on the main loop iteration that GTK's `Application::run` is
//! draining anyway. The result: whatever happens on a button click
//! or text-field edit can update signals, fire effects, and mutate
//! widget state — all on the main thread, where GTK requires it.
//!
//! # Lifecycle
//!
//! Call [`init`] once, before constructing the first signal/effect.
//! It's idempotent — second and later calls return
//! `Err(ExecutorError::AlreadySet)` and can be ignored.

use any_spawner::{
    CustomExecutor, Executor, ExecutorError, PinnedFuture, PinnedLocalFuture,
};
use gtk4::glib::MainContext;

/// Initialise the global GTK main-thread executor. Idempotent.
///
/// Must be called from the GTK main thread.
pub fn init() -> Result<(), ExecutorError> {
    Executor::init_custom_executor(GtkExecutor)
}

/// Marker type. The actual scheduling logic is in the
/// [`CustomExecutor`] impl below.
struct GtkExecutor;

impl CustomExecutor for GtkExecutor {
    fn spawn(&self, fut: PinnedFuture<()>) {
        // PinnedFuture is Send. We don't need that property because
        // GTK is single-threaded; routing through `spawn_local` is
        // fine and avoids a second code path.
        MainContext::default().spawn_local(fut);
    }

    fn spawn_local(&self, fut: PinnedLocalFuture<()>) {
        MainContext::default().spawn_local(fut);
    }

    fn poll_local(&self) {
        // glib drives polling via its main loop; nothing to drain
        // manually.
    }
}
