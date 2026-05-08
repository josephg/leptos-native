//! Repeating-timer support — the iOS analog of web's
//! `set_interval_with_handle` / `clearInterval`.
//!
//! Backed by `NSTimer::scheduledTimerWithTimeInterval:...`,
//! which adds the timer to the main run loop and fires the
//! selector on the given target every `interval` seconds.
//!
//! # Threading
//!
//! Must be called from the main thread (NSTimer's run-loop is
//! the main one). The returned [`IntervalHandle`] is `Send` via
//! `SendWrapper` for parity with web's API but only usable on
//! main.

use crate::MainThreadMarker;
use objc2::{
    define_class, msg_send, rc::Retained, runtime::NSObject, sel,
    DefinedClass, MainThreadOnly,
};
use objc2_foundation::NSTimer;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, time::Duration};

type Callback = RefCell<Box<dyn FnMut() + 'static>>;

define_class!(
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
                        "[ios_dom] reentrant interval callback skipped"
                    );
                    return;
                }
            };
            (cb)();
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

#[derive(Clone)]
pub struct IntervalHandle {
    timer: SendWrapper<Retained<NSTimer>>,
}

impl IntervalHandle {
    pub fn clear(&self) {
        self.timer.invalidate();
    }
}

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

    Ok(IntervalHandle {
        timer: SendWrapper::new(timer),
    })
}

pub fn set_interval(cb: impl FnMut() + 'static, duration: Duration) {
    let _ = set_interval_with_handle(cb, duration);
}
