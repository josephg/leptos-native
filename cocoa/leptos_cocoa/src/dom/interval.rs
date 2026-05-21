//! Repeating-timer support — the macOS analog of web's
//! `set_interval_with_handle` / `clearInterval`.
//!
//! Backed by `NSTimer::scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:`,
//! which adds the timer to the main run loop and fires the
//! selector on the given target every `interval` seconds. We
//! supply our own ObjC subclass (`TimerTarget`) that holds a
//! Rust closure as an ivar; the selector invokes the closure.
//!
//! Cancellation is via `IntervalHandle::clear()`, which calls
//! `[timer invalidate]` — AppKit then drops its retain on the
//! target, which drops the closure.
//!
//! # Threading
//!
//! Must be called from the main thread (NSTimer's run-loop is
//! the main one). The returned [`IntervalHandle`] is `Send` via
//! `SendWrapper` for parity with web's API but only usable on
//! main.

use super::MainThreadMarker;
use objc2::{
    define_class, msg_send, rc::Retained, runtime::NSObject, sel,
    DefinedClass, MainThreadOnly,
};
use objc2_foundation::NSTimer;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, time::Duration};

type Callback = RefCell<Box<dyn FnMut() + 'static>>;

define_class!(
    /// ObjC class that holds a Rust closure as an ivar and exposes
    /// one selector — `fire:` — which the NSTimer calls on each
    /// tick.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Callback]
    pub struct TimerTarget;

    impl TimerTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, _timer: *mut NSObject) {
            let mut cb = match self.ivars().try_borrow_mut() {
                Ok(cb) => cb,
                Err(_) => {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "[cocoa_dom] reentrant interval callback skipped"
                    );
                    return;
                }
            };
            cb();
        }
    }
);

impl TimerTarget {
    fn new(cb: impl FnMut() + 'static, mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm);
        let this = alloc.set_ivars(RefCell::new(Box::new(cb)));
        unsafe { msg_send![super(this), init] }
    }
}

/// Cancelable interval handle. Returned by
/// [`set_interval_with_handle`]; call [`clear`](Self::clear) to
/// stop the timer.
#[derive(Clone)]
pub struct IntervalHandle {
    timer: SendWrapper<Retained<NSTimer>>,
}

impl IntervalHandle {
    /// Stop firing. Idempotent — calling twice is fine. Subsequent
    /// run-loop ticks won't invoke the callback.
    pub fn clear(&self) {
        self.timer.invalidate();
    }
}

/// Error type for [`set_interval_with_handle`]. Currently only
/// fires if called off the main thread; future failure modes can
/// extend this.
#[derive(Debug)]
pub enum IntervalError {
    NotMainThread,
}

impl std::fmt::Display for IntervalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntervalError::NotMainThread => write!(
                f,
                "set_interval_with_handle must be called on the main thread"
            ),
        }
    }
}

impl std::error::Error for IntervalError {}

/// Schedule `cb` to fire repeatedly with the given interval, on
/// the main run loop. Returns a handle that cancels the timer
/// when [`clear`](IntervalHandle::clear) is called.
///
/// Mirrors the web `leptos_dom::helpers::set_interval_with_handle`
/// signature (returning `Result<IntervalHandle, _>`) so the same
/// example code compiles unchanged.
pub fn set_interval_with_handle(
    cb: impl FnMut() + 'static,
    duration: Duration,
) -> Result<IntervalHandle, IntervalError> {
    let mtm =
        MainThreadMarker::new().ok_or(IntervalError::NotMainThread)?;

    let target = TimerTarget::new(cb, mtm);
    let target_obj: &objc2::runtime::AnyObject = &target;

    let timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            duration.as_secs_f64(),
            target_obj,
            sel!(fire:),
            None,
            true,
        )
    };
    // NSTimer retains its target; we don't need to keep our own
    // reference to `target` separately. When invalidate fires, the
    // target's retain is released.

    Ok(IntervalHandle {
        timer: SendWrapper::new(timer),
    })
}

/// Convenience wrapper — same shape as web's `set_interval`. Just
/// drops the handle.
pub fn set_interval(cb: impl FnMut() + 'static, duration: Duration) {
    let _ = set_interval_with_handle(cb, duration);
}
