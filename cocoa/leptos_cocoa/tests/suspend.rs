//! Regression tests for `Suspend<F>` (`common/leptos/src/suspend.rs`).
//!
//! Two bugs landed during the recent code-review pass that these
//! tests pin in place:
//!
//!   1. **Placeholder position**: when the future resolves, the
//!      built view must be spliced at the placeholder's current
//!      position, not at the original `mount()` marker. Previously
//!      the resolved view re-appeared at the *end* of the parent
//!      (using the parent + None marker) instead of in the slot
//!      where the placeholder lived.
//!   2. **Orphan-task cleanup**: when `SuspendState` is dropped
//!      before the future resolves, the spawned task's eventual
//!      `view.build()` must still have its `unmount()` driven so
//!      handler-store entries and similar RAII bookkeeping clear
//!      up. Previously the task held a strong `Rc` and just
//!      dropped the freshly-built state, skipping unmount.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

mod common;

use leptos_cocoa::dom::event::handler_store_size_for_test;
use leptos_native::prelude::*;
use leptos_cocoa::cocoa::element::{button, hstack, label};
use leptos_cocoa::event_macos::{click, on};
use leptos_cocoa::CocoaDom;
use reactive_graph::owner::Owner;
use leptos_native::renderer::view::{AddAnyAttr, Mountable, Render};
use std::cell::RefCell;
use std::rc::Rc;
use leptos_cocoa::dom::window::open_window;

fn with_reactive_scope<F: FnOnce()>(body: F) {
    let _ = leptos_cocoa::dom::spawner::init().unwrap();
    let owner = Owner::new();
    owner.with(body);
}

/// Minimal main-thread oneshot. Pure `Rc<RefCell<…>>` — the
/// Suspend future polls it via a `std::future::poll_fn` driven by
/// `Executor::poll`-equivalent dispatch ticks.
#[derive(Clone)]
struct MainThreadGate {
    inner: Rc<RefCell<GateState>>,
}

#[derive(Default)]
struct GateState {
    ready: bool,
    waker: Option<std::task::Waker>,
}

impl MainThreadGate {
    fn new() -> Self {
        Self { inner: Rc::new(RefCell::new(GateState::default())) }
    }

    fn release(&self) {
        let waker = {
            let mut g = self.inner.borrow_mut();
            g.ready = true;
            g.waker.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
    }

    fn wait(&self) -> impl std::future::Future<Output = ()> {
        let inner = self.inner.clone();
        std::future::poll_fn(move |cx| {
            let mut g = inner.borrow_mut();
            if g.ready {
                std::task::Poll::Ready(())
            } else {
                g.waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            }
        })
    }
}

// ---------------------------------------------------------------------
// 1. Resolved view splices at the placeholder's position, not at
//    the original mount marker.
// ---------------------------------------------------------------------
//
// Layout: hstack of three siblings — label("a"), Suspend(label("b")),
// label("c"). Before resolution, the middle subview is the
// placeholder. After resolution, subview[1] should become label("b")
// — not appended at the end (which is what the bug-shape would
// produce, leaving order [a, c, b]).

fn suspend_splices_at_placeholder_position() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let opened = open_window("suspend-position", (640.0, 480.0), mtm);

        let gate = MainThreadGate::new();
        let gate_for_future = gate.clone();
        let view = hstack()
            .child(label().child("a"))
            .child(Suspend::new(async move {
                gate_for_future.wait().await;
                label().child("b")
            }))
            .child(label().child("c"));

        let mut state = <_ as Render<CocoaDom>>::build(view);
        state.mount(opened.content_root, None);

        // Pump once to let initial mount settle. The Suspend
        // future is still pending (gate not released), so the
        // middle slot is the placeholder NSView.
        common::pump_run_loop(0.05);

        let parent_subviews_before = opened.content_root.ns_view();
        let hstack_view = {
            use objc2::rc::Retained;
            let sv = parent_subviews_before.subviews();
            assert!(sv.len() >= 1, "content root should have the hstack");
            let h: Retained<objc2_app_kit::NSView> = sv.objectAtIndex(0);
            h
        };
        let before = hstack_view.subviews();
        assert_eq!(
            before.len(),
            3,
            "hstack should have three children pre-resolution \
             (label-a, placeholder, label-c)"
        );

        // Release the gate, pump until the spawned task runs and
        // the splice completes.
        gate.release();
        common::pump_run_loop(0.1);

        let after = hstack_view.subviews();
        assert_eq!(
            after.len(),
            3,
            "hstack should still have three children post-resolution"
        );

        // The middle subview should now be an NSTextField (label),
        // not the placeholder (NSView). We can't directly identify
        // 'b' without setting an accessibility identifier, so we
        // assert the middle subview is an NSTextField — the
        // placeholder is a plain NSView, the label is a
        // NSTextField. If the bug were present, subview[1] would
        // still be the placeholder NSView and subview[2] (or a
        // hypothetical subview[3]) would be the label.
        let middle = after.objectAtIndex(1);
        let is_text_field: bool = unsafe {
            use objc2::msg_send;
            use objc2::runtime::AnyClass;
            let cls = AnyClass::get(c"NSTextField").unwrap();
            let raw: &objc2_app_kit::NSView = &*middle;
            let raw_any: &objc2::runtime::AnyObject = raw.as_ref();
            let r: objc2::runtime::Bool =
                msg_send![raw_any, isKindOfClass: cls];
            r.as_bool()
        };
        assert!(
            is_text_field,
            "after resolution the middle slot should hold the \
             resolved label (NSTextField), not a placeholder NSView. \
             Regression: Suspend splice ignored placeholder position."
        );

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 2. Orphan cleanup: dropping SuspendState before the future
//    resolves still drives unmount() on the eventually-built view.
// ---------------------------------------------------------------------
//
// Strategy: the resolved view is a Button with an on:click
// handler. Mounting a button writes one entry into the cocoa
// HANDLER_STORE; unmount drops it. If we drop the SuspendState
// while pending, then resolve, the orphan path inside Suspend's
// spawned task must call `state.unmount()` — otherwise the handler
// store size grows by one and never recovers.

fn suspend_orphan_cleanup_unmounts_built_view() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let opened =
            open_window("suspend-orphan", (640.0, 480.0), mtm);

        let baseline = handler_store_size_for_test();

        let gate = MainThreadGate::new();
        let gate_for_future = gate.clone();
        let view = Suspend::new(async move {
            gate_for_future.wait().await;
            button().title("click me").add_any_attr(on(click, |_| {}))
        });

        let mut state = <_ as Render<CocoaDom>>::build(view);
        state.mount(opened.content_root, None);
        common::pump_run_loop(0.05);

        // Drop the state BEFORE the future resolves.
        drop(state);

        // Now release the gate. The spawned task will resolve,
        // call `view.build()` (registering a handler), then find
        // `inner_weak.upgrade()` returns None and explicitly
        // unmount the freshly-built state — which drops the
        // handler-store entry.
        gate.release();
        common::pump_run_loop(0.1);

        let after = handler_store_size_for_test();
        assert_eq!(
            after,
            baseline,
            "orphan-resolved Suspend must unmount the built view \
             so HANDLER_STORE returns to its pre-Suspend baseline. \
             Regression: orphan path skipped unmount() and leaked \
             the handler entry."
        );

        std::mem::forget(opened);
    });
}

fn main() {
    common::run_tests(&[
        (
            "suspend_splices_at_placeholder_position",
            suspend_splices_at_placeholder_position,
        ),
        (
            "suspend_orphan_cleanup_unmounts_built_view",
            suspend_orphan_cleanup_unmounts_built_view,
        ),
    ]);
}
