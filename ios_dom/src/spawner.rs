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
    future: SendWrapper<RefCell<Option<TaskFuture>>>,
    queued: AtomicBool,
}

impl Task {
    fn new(fut: TaskFuture) -> Arc<Self> {
        Arc::new(Task {
            future: SendWrapper::new(RefCell::new(Some(fut))),
            queued: AtomicBool::new(true),
        })
    }

    fn poll_on_main(self: Arc<Self>) {
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
