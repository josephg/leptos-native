//! Layout regression tests for the leptos_cocoa builders.
//!
//! The motivating test is `leaf_controls_have_nonzero_intrinsic_size` —
//! a regression test for the bug where leaf controls (Button, Label,
//! TextField, etc.) ended up with zero-sized frames because their
//! `Render::build` was creating a `UnitState` placeholder as a child,
//! making them non-leaves in Taffy and bypassing the
//! `intrinsicContentSize` measure callback.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::cocoa::element::{button, hstack, label, vstack};
use leptos_cocoa::dom::{layout, spawner, window, CocoaNode};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSButton, NSTextField};
use reactive_graph::owner::Owner;
use renderer::attrs::WithLayout;
use renderer::view::{Mountable, Render};

/// Spin up a fresh reactive owner + the spawner the cocoa effect
/// machinery needs for build/mount.
fn with_reactive_scope<F: FnOnce()>(body: F) {
    let _ = spawner::init().unwrap();
    let owner = Owner::new();
    owner.with(body);
}

/// Open a real NSWindow (off-screen, never `makeKeyAndOrderFront`'d),
/// build `view`, mount it under the window's content_root, run
/// compute_layout, and call `f` with the content_root. The window
/// stays alive until the closure returns.
fn with_mounted_view<V, F>(view: V, size: (f64, f64), f: F)
where
    V: Render<leptos_cocoa::Dom>,
    V::State: Mountable<leptos_cocoa::Dom>,
    F: FnOnce(&CocoaNode),
{
    let mtm = common::test_mtm();
    let opened = window::open_window("test", size, mtm);
    let mut state = view.build();
    state.mount(opened.content_root, None);
    let content_size = opened.content_root.ns_view().frame().size;
    layout::compute_layout(
        opened.content_root,
        content_size,
    );
    f(&opened.content_root);
    // Don't close the window here — `OpenedWindow::Drop` releases it.
    drop(state);
    drop(opened);
}

/// Recursively walk the NSView tree and call `f` on every NSView. Used
/// to find specific control subclasses by class downcast.
fn walk(view: &objc2_app_kit::NSView, f: &mut impl FnMut(&objc2_app_kit::NSView)) {
    f(view);
    for sv in view.subviews().iter() {
        walk(&*sv, f);
    }
}

/// REGRESSION: leaf controls (button, label) must have non-zero
/// intrinsic-sized frames after compute_layout.
///
/// Bug: leaf builders' `Render::build` set
/// `type State = ElementState<(), UnitState<Dom>>` and
/// `children: <() as Render<Dom>>::build(())`, which mounted a
/// placeholder NSView under the leaf NSButton/NSTextField. Once the
/// leaf had a child, Taffy treated it as a container and skipped the
/// `intrinsicContentSize` measure callback — frames came out 0×0 and
/// the window rendered blank.
///
/// Fix: changed leaf State to `ElementState<(), ()>` + `children: ()`,
/// backed by a no-op `Mountable<R> for ()` impl.
fn leaf_controls_have_nonzero_intrinsic_size() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = vstack().padding(16.0).gap(12.0).child(
            label().text("Hello"),
        );
        with_mounted_view(view, (320.0, 200.0), |root| {
            let mut found_field = false;
            walk(&root.ns_view(), &mut |v| {
                let any: &AnyObject = v.as_ref();
                if let Some(field) = any.downcast_ref::<NSTextField>() {
                    found_field = true;
                    let frame = field.frame();
                    assert!(
                        frame.size.height > 0.0,
                        "label frame has zero height: {:?}",
                        frame
                    );
                    assert!(
                        frame.size.width > 0.0,
                        "label frame has zero width: {:?}",
                        frame
                    );
                }
            });
            assert!(found_field, "no NSTextField in subview tree");
        });
    });
}

/// Buttons in an hstack should each get their natural intrinsic
/// width and a height of ~32 (the macOS push button bezel).
fn button_in_hstack_has_natural_size() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = hstack()
            .gap(8.0)
            .child(button().title("OK"))
            .child(button().title("Cancel"));
        with_mounted_view(view, (320.0, 100.0), |root| {
            let mut button_frames = Vec::new();
            walk(&root.ns_view(), &mut |v| {
                let any: &AnyObject = v.as_ref();
                if any.downcast_ref::<NSButton>().is_some() {
                    button_frames.push(v.frame());
                }
            });
            assert_eq!(
                button_frames.len(),
                2,
                "expected 2 buttons in tree, got {}",
                button_frames.len()
            );
            for (i, f) in button_frames.iter().enumerate() {
                assert!(
                    f.size.height >= 28.0,
                    "button[{i}] height {} too small (expected ~32)",
                    f.size.height
                );
                assert!(
                    f.size.width > 10.0,
                    "button[{i}] width {} too small (expected natural width)",
                    f.size.width
                );
            }
        });
    });
}

/// Vstack containing a label + hstack of buttons should produce a
/// non-zero composite height — exercises the cascade through every
/// layer (vstack → label, vstack → hstack → buttons).
fn vstack_label_plus_hstack_has_full_height() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = vstack()
            .padding(16.0)
            .gap(12.0)
            .child(label().text("Count: 0"))
            .child(
                hstack()
                    .gap(8.0)
                    .child(button().title("-1"))
                    .child(button().title("Reset"))
                    .child(button().title("+1")),
            );
        with_mounted_view(view, (320.0, 200.0), |root| {
            let subs = root.ns_view().subviews();
            assert!(subs.len() > 0, "content_root has no subviews");
            let outer = subs.iter().next().expect("subview");
            let frame = outer.frame();
            // padding(16)*2 = 32, label ~16, gap 12, button ~32 → ~92
            // total. Allow some slop; main thing is it's not 0.
            assert!(
                frame.size.height >= 60.0,
                "vstack height {} suspiciously small (expected >= ~60)",
                frame.size.height
            );
            assert_eq!(
                frame.size.width, 320.0,
                "vstack should fill its parent width"
            );
        });
    });
}

/// REGRESSION: `hidden=true` should collapse the Taffy slot (CSS
/// `display: none` semantics), not just hide the view while keeping
/// the space reserved. Pre-P1 behaviour was a footgun — looked right
/// but reserved space; users had to reach for `<Show>`.
fn hidden_collapses_layout_slot() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // Two siblings in a vstack — second is hidden=true. The vstack's
        // total height should be ~ label height (+ padding), NOT label
        // + label + gap.
        let view = vstack()
            .padding(0.0)
            .gap(8.0)
            .child(label().text("visible"))
            .child(label().text("collapsed").hidden(true));
        with_mounted_view(view, (320.0, 200.0), |root| {
            let subs = root.ns_view().subviews();
            let outer = subs.iter().next().expect("vstack subview");
            let frame = outer.frame();
            // A single label intrinsic-sizes to ~16-18pt. Two labels +
            // 8pt gap would be ~40+. Allow generous slop, but assert
            // strictly less than the two-label total.
            assert!(
                frame.size.height < 30.0,
                "hidden child wasn't collapsed: vstack height = {} \
                 (expected ~one-label, ~18; two-label-with-gap would \
                 be ~40+)",
                frame.size.height,
            );
        });
    });
}

// Removed: `toolbar_keeps_explicit_height` tested the old hstack-
// shaped `toolbar()` builder. The current `<toolbar>` is backed
// by NSToolbar and lives outside the layout tree — it doesn't
// have a Taffy height to check. See
// `cocoa/leptos_cocoa/tests/toolbar.rs` for the NSToolbar tests.

fn main() {
    common::run_tests(&[
        (
            "leaf_controls_have_nonzero_intrinsic_size",
            leaf_controls_have_nonzero_intrinsic_size,
        ),
        (
            "button_in_hstack_has_natural_size",
            button_in_hstack_has_natural_size,
        ),
        (
            "vstack_label_plus_hstack_has_full_height",
            vstack_label_plus_hstack_has_full_height,
        ),
        ("hidden_collapses_layout_slot", hidden_collapses_layout_slot),
    ]);
}
