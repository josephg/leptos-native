//! UIKit main-thread executor for `any_spawner`.
//!
//! reactive_graph uses `any_spawner::Executor::spawn_local` to drive
//! effects (each Effect spawns a tiny async task that loops on a
//! notification channel and re-runs the effect body). Without an
//! executor wired in, signal updates never trigger effect runs and the
//! UI never updates.
//!
//! This executor schedules every poll on libdispatch's main queue,
//! which is the queue that UIKit's run loop drains. The result:
//! whatever happens on a button tap or text-field edit can update
//! signals, fire effects, and mutate UIView state — all on the main
//! thread, where UIKit requires it.
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

/// Initialise the global UIKit main-thread executor. Idempotent.
///
/// Must be called from the main thread.
pub fn init() -> Result<(), ExecutorError> {
    Executor::init_custom_executor(UIKitExecutor)
}

/// Marker type. The actual scheduling logic is in the [`CustomExecutor`]
/// impl below.
struct UIKitExecutor;

impl CustomExecutor for UIKitExecutor {
    fn spawn(&self, fut: PinnedFuture<()>) {
        spawn_main(TaskFuture::Send(fut));
    }

    fn spawn_local(&self, fut: PinnedLocalFuture<()>) {
        spawn_main(TaskFuture::Local(SendWrapper::new(fut)));
    }

    fn poll_local(&self) {
        // Tasks are dispatch-driven; there's nothing to drain here.
    }
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
    /// `Option` so [`Drop`] can move the wrapper out and defer its
    /// destruction to the main queue — see the `Drop` impl. Always
    /// `Some` while the task is live.
    future: Option<SendWrapper<RefCell<Option<TaskFuture>>>>,
    queued: AtomicBool,
}

impl Task {
    fn new(fut: TaskFuture) -> Arc<Self> {
        Arc::new(Task {
            future: Some(SendWrapper::new(RefCell::new(Some(fut)))),
            queued: AtomicBool::new(true),
        })
    }

    fn poll_on_main(self: Arc<Self>) {
        self.queued.store(false, Ordering::Release);

        let waker: Waker = self.clone().into();
        let mut cx = Context::from_waker(&waker);

        let Some(future) = self.future.as_ref() else {
            return;
        };
        let mut slot = future.borrow_mut();
        if let Some(fut) = slot.as_mut() {
            match fut.poll(&mut cx) {
                Poll::Ready(()) => *slot = None,
                Poll::Pending => { /* will be re-polled by waker */ }
            }
        }
    }
}

impl Drop for Task {
    /// The task's `Waker` is a clone of the owning `Arc<Task>`, and
    /// wakers legitimately travel to other threads (a `tokio::spawn`ed
    /// job holds one registered by a bridged `JoinHandle`, then wakes
    /// it from a worker thread). If such a thread drops the LAST
    /// `Arc<Task>` reference — e.g. the final main-queue poll finishes
    /// while the woken thread is still inside `wake()` — this Drop
    /// runs off-main, and letting the `SendWrapper` field drop there
    /// panics ("Dropped SendWrapper<T> variable from a thread
    /// different to the one it has been created with"), which under
    /// `panic = "abort"` kills the process. Ship the wrapper back to
    /// the main queue instead; `SendWrapper<T>` is `Send`, and its
    /// contents are only ever touched (and now destroyed) on main.
    fn drop(&mut self) {
        if let Some(fut) = self.future.take() {
            if !fut.valid() {
                DispatchQueue::main().exec_async(move || drop(fut));
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
            DispatchQueue::main().exec_async(move || task.poll_on_main());
        }
    }
}

fn spawn_main(fut: TaskFuture) {
    let task = Task::new(fut);
    DispatchQueue::main().exec_async(move || task.poll_on_main());
}
