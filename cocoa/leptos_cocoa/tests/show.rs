//! `<Show>` control-flow regression tests.
//!
//! In particular the no-fallback flip path: when `<Show when=cond>`
//! has no `fallback`, the when=false state is `Either::Right(())`.
//! Either::rebuild calls `old.insert_before_this(&mut new_state)`
//! and ignores the return value — but `()`'s `insert_before_this`
//! returns false WITHOUT mounting the new state. So flipping
//! when=false → true silently fails to mount the child, and the
//! user's view appears empty even though the condition is true.

#![cfg(target_os = "macos")]

mod common;

use leptos::children::ToChildren;
use leptos::control_flow::Show;
use leptos_cocoa::cocoa::element::label;
use leptos_cocoa::Dom;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, Set};
use renderer::view::{Mountable, Render};

fn with_reactive_scope<F: FnOnce()>(f: F) {
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

/// Show with no fallback, starting false, then flipping to true.
/// The child label should appear among the parent's subviews after
/// the flip. Regression: previously the label was built but never
/// mounted because Either::rebuild ignored the
/// `insert_before_this` return.
fn show_without_fallback_mounts_on_flip_false_to_true() {
    use cocoa_dom::window::open_window;

    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let opened =
            open_window("show-no-fallback", (640.0, 480.0), mtm);

        let when = RwSignal::new(false);
        // ShowProps without `fallback` — the Show component returns
        // `Either::Right(())` for the empty branch.
        // The Fb generic on ShowProps is normally inferred from
        // the fallback argument; with no fallback, pin it
        // explicitly with the same shape ShowEmpty would expose
        // (`Label` is a concrete IntoView).
        type Fb = leptos_cocoa::cocoa::element::Label;
        let view = leptos::control_flow::Show::<_, _, Fb, Dom>(
            leptos::control_flow::ShowProps::<_, _, Fb, Dom>::builder()
                .when(move || when.get())
                .children(ToChildren::to_children(|| label().text("hello")))
                .build(),
        );

        let mut state = <_ as Render<Dom>>::build(view);
        state.mount(&opened.content_root, None);
        common::pump_run_loop(0.05);

        // Initial: when=false → empty. Parent should have the
        // placeholder subview (() State, or a UnitState) but no
        // NSTextField.
        let parent = opened.content_root.ns_view();

        // Flip true.
        when.set(true);
        common::pump_run_loop(0.1);

        let subs = parent.subviews();
        let mut found_label = false;
        for i in 0..subs.len() {
            let s = subs.objectAtIndex(i);
            let is_text_field: bool = unsafe {
                use objc2::msg_send;
                use objc2::runtime::AnyClass;
                let cls = AnyClass::get(c"NSTextField").unwrap();
                let raw: &objc2_app_kit::NSView = &*s;
                let raw_any: &objc2::runtime::AnyObject = raw.as_ref();
                let r: objc2::runtime::Bool =
                    msg_send![raw_any, isKindOfClass: cls];
                r.as_bool()
            };
            if is_text_field {
                found_label = true;
                break;
            }
        }

        assert!(
            found_label,
            "after flipping Show.when false → true (with no fallback), \
             the child label must be mounted. Regression: \
             Either::rebuild's `old.insert_before_this(&mut new)` \
             returns false for `()` and the new state is silently \
             abandoned without being mounted."
        );

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

fn main() {
    common::run_tests(&[(
        "show_without_fallback_mounts_on_flip_false_to_true",
        show_without_fallback_mounts_on_flip_false_to_true,
    )]);
}
