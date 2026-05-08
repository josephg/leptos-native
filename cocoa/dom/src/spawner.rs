//! AppKit main-thread executor for `any_spawner`.
//!
//! Reactive_graph uses `any_spawner::Executor::spawn_local` to drive
//! effects (each Effect spawns a tiny async task that loops on a
//! notification channel and re-runs the effect body). Without an
//! executor wired in, signal updates never trigger effect runs and the
//! UI never updates.
//!
//! This executor schedules every poll on libdispatch's main queue,
//! which is the queue that AppKit's run loop drains. The result:
//! whatever happens on a button click or text-field edit can update
//! signals, fire effects, and mutate NSView state — all on the main
//! thread, where AppKit requires it.
//!
//! # Lifecycle
//!
//! Call [`init`] once, before constructing the first signal/effect.
//! It's idempotent — second and later calls return
//! `Err(ExecutorError::AlreadySet)` and can be ignored.

use any_spawner::{
    CustomExecutor, Executor, ExecutorError, PinnedFuture, PinnedLocalFuture,
};
use dispatch2::DispatchQueue;
use send_wrapper::SendWrapper;
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
};

/// Initialise the global AppKit main-thread executor. Idempotent.
///
/// Must be called from the main thread.
pub fn init() -> Result<(), ExecutorError> {
    Executor::init_custom_executor(AppKitExecutor)
}

/// Marker type. The actual scheduling logic is in the [`CustomExecutor`]
/// impl below.
struct AppKitExecutor;

impl CustomExecutor for AppKitExecutor {
    fn spawn(&self, fut: PinnedFuture<()>) {
        spawn_main(TaskFuture::Send(fut));
    }

    fn spawn_local(&self, fut: PinnedLocalFuture<()>) {
        // PinnedLocalFuture is `!Send`; SendWrapper carries it across
        // the dispatch boundary with a runtime check that we never
        // actually use it off-main.
        spawn_main(TaskFuture::Local(SendWrapper::new(fut)));
    }

    fn poll_local(&self) {
        // Tasks are dispatch-driven; there's nothing to drain here.
    }
}

/// Internal uniform representation of a spawned task. Both variants
/// hold a `Pin<Box<dyn Future>>` whose box address is stable across
/// polls, so we can call `.as_mut().poll(cx)` on them directly without
/// re-pinning anything ourselves. The variants differ only in Send-ness
/// of the inner future.
enum TaskFuture {
    Send(PinnedFuture<()>),
    Local(SendWrapper<PinnedLocalFuture<()>>),
}

impl TaskFuture {
    /// Poll the inner future. Both variants forward to the inner
    /// `Pin<Box<...>>::as_mut().poll(cx)` — the pinning lives entirely
    /// inside the boxed future, which is heap-allocated and never
    /// moves, so there's no `unsafe` here.
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        match self {
            TaskFuture::Send(f) => f.as_mut().poll(cx),
            TaskFuture::Local(f) => f.as_mut().poll(cx),
        }
    }
}

struct Task {
    /// SendWrapper is needed because the main-queue closure must be
    /// `Send + 'static`, but our future may not be `Send`. The wrapper
    /// asserts at runtime that we only access it on the thread that
    /// constructed it (always main, by construction).
    future: SendWrapper<RefCell<Option<TaskFuture>>>,
    /// Set whenever `wake` is called between polls. Used to coalesce
    /// repeated wakes into a single re-poll dispatch.
    queued: AtomicBool,
}

impl Task {
    fn new(fut: TaskFuture) -> Arc<Self> {
        Arc::new(Task {
            future: SendWrapper::new(RefCell::new(Some(fut))),
            // Mark as already queued — `spawn_main` is about to enqueue
            // the first poll itself, so additional wake() calls before
            // that first poll runs would otherwise enqueue a duplicate.
            queued: AtomicBool::new(true),
        })
    }

    fn poll_on_main(self: Arc<Self>) {
        // Clear the "queued" flag *before* polling, so any wake() that
        // fires during the poll itself enqueues a follow-up poll
        // instead of being silently coalesced away.
        self.queued.store(false, Ordering::Release);

        let waker: Waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);

        let mut slot = self.future.borrow_mut();
        if let Some(fut) = slot.as_mut() {
            match fut.poll(&mut cx) {
                Poll::Ready(()) => *slot = None,
                Poll::Pending => { /* will be re-polled by waker */ }
            }
        }
    }
}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        Self::wake_by_ref(&self);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        // Coalesce: if a poll is already queued, do nothing. Otherwise
        // queue a fresh dispatch onto the main queue.
        if self
            .queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let task = Arc::clone(self);
            DispatchQueue::main().exec_async(move || task.poll_on_main());
        }
    }
}

fn spawn_main(fut: TaskFuture) {
    let task = Task::new(fut);
    DispatchQueue::main().exec_async(move || task.poll_on_main());
}
