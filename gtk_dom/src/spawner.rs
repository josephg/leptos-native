//! GTK main-thread executor for `any_spawner`.
//!
//! Reactive_graph uses `any_spawner::Executor::spawn_local` to drive
//! effects. Each effect spawns a tiny async task that loops on a
//! notification channel and re-runs the effect body.
//!
//! We use `glib::idle_add_local` to schedule polls on the GTK main
//! loop. It always defers (unlike `MainContext::invoke`, which runs
//! inline when the thread owns the context — deadlocking if called
//! during GTK signal dispatch). A manual `Wake` impl coalesces
//! repeated wakes via an atomic flag so only one idle source is
//! queued at a time.
//!
//! `idle_add_local` returns a `SourceId` whose `Drop` calls
//! `g_source_remove`. If dropped before dispatch, the callback
//! never fires. We store each `SourceId` in the `Task` itself; when
//! the callback fires (returns `Break`), GTK auto-removes the
//! source, so dropping our stored `SourceId` is a harmless no-op.
//! When the future completes (`Poll::Ready`), the `Task` (and its
//! source) is cleaned up naturally — no accumulation.
//!
//! # Lifecycle
//!
//! Call [`init`] once, before constructing the first signal/effect.
//! It's idempotent.

use any_spawner::{
    CustomExecutor, Executor, ExecutorError, PinnedFuture, PinnedLocalFuture,
};
use send_wrapper::SendWrapper;
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Wake, Waker},
};

/// Initialise the global GTK main-thread executor. Idempotent.
pub fn init() -> Result<(), ExecutorError> {
    Executor::init_custom_executor(GtkExecutor)
}

struct GtkExecutor;

impl CustomExecutor for GtkExecutor {
    fn spawn(&self, fut: PinnedFuture<()>) {
        spawn_on_main(TaskFuture::Send(fut));
    }

    fn spawn_local(&self, fut: PinnedLocalFuture<()>) {
        spawn_on_main(TaskFuture::Local(SendWrapper::new(fut)));
    }

    fn poll_local(&self) {}
}

enum TaskFuture {
    Send(PinnedFuture<()>),
    Local(SendWrapper<PinnedLocalFuture<()>>),
}

impl TaskFuture {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match self {
            TaskFuture::Send(f) => f.as_mut().poll(cx),
            TaskFuture::Local(f) => f.as_mut().poll(cx),
        }
    }
}

struct Task {
    future: SendWrapper<RefCell<Option<TaskFuture>>>,
    queued: AtomicBool,
    /// Held alive so the idle source isn't removed before dispatch.
    /// When the callback fires (returns `Break`), GTK removes the
    /// source automatically; dropping this `SourceId` afterwards is
    /// a safe no-op.
    source: Mutex<Option<glib::SourceId>>,
}

impl Task {
    fn new(fut: TaskFuture) -> Arc<Self> {
        Arc::new(Task {
            future: SendWrapper::new(RefCell::new(Some(fut))),
            queued: AtomicBool::new(true),
            source: Mutex::new(None),
        })
    }

    fn poll_on_main(self: &Arc<Self>) {
        self.queued.store(false, Ordering::Release);

        let waker: Waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);

        let mut slot = self.future.borrow_mut();
        if let Some(fut) = slot.as_mut() {
            match fut.poll(&mut cx) {
                Poll::Ready(()) => {
                    *slot = None;
                    // Future done — drop the source ID (safe:
                    // GTK already removed it when we returned Break).
                    *self.source.lock().unwrap() = None;
                }
                Poll::Pending => { /* re-polled when waker fires */ }
            }
        }
    }
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        if self
            .queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let task = Arc::clone(self);
            let source_id = {
                let t = Arc::clone(&task);
                glib::idle_add_local(move || {
                    t.poll_on_main();
                    glib::ControlFlow::Break
                })
            };
            *task.source.lock().unwrap() = Some(source_id);
        }
    }
}

fn spawn_on_main(fut: TaskFuture) {
    let task = Task::new(fut);
    let source_id = {
        let t = Arc::clone(&task);
        glib::idle_add_local(move || {
            t.poll_on_main();
            glib::ControlFlow::Break
        })
    };
    *task.source.lock().unwrap() = Some(source_id);
}
