//! `<split_view>` / `<split_pane>` integration tests.
//!
//! These hit AppKit directly: opening windows, building real
//! NSSplitViewController + NSSplitViewItem chains, mounting view
//! trees into pane roots, and exercising the reactive-collapsed
//! wiring. The pure-Rust trait-level tests live in
//! `cocoa/leptos_cocoa/src/cocoa/split.rs::tests`.

#![cfg(target_os = "macos")]

extern crate leptos_cocoa as leptos_platform;

mod common;

use leptos_cocoa::dom::split_window::{
    open_split_window, OpenedSplitWindow, PaneBehavior, PaneSpec,
};
use leptos_cocoa::cocoa::element::label;
use leptos_cocoa::dom::CocoaNodeExt;
use leptos_cocoa::cocoa::split::{
    split_pane, split_view, IntoSplitView,
};
use objc2::msg_send;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, Set};
use leptos_native::prelude::Mountable;
use leptos_native::renderer::view::Render;

fn with_reactive_scope<F: FnOnce()>(f: F) {
    // `init` is process-global; the custom harness runs every test in one
    // process, so only the first call succeeds. Ignore the `AlreadySet` the
    // rest return — it just means the executor is already wired up.
    let _ = leptos_cocoa::dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

// ---------------------------------------------------------------------
// 1. open_split_window — basic shape
// ---------------------------------------------------------------------

fn open_split_window_with_two_panes_yields_two_panes() {
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec { behavior: PaneBehavior::Sidebar,   ..Default::default() },
        PaneSpec { behavior: PaneBehavior::Default,   ..Default::default() },
    ];
    let opened: OpenedSplitWindow =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);

    assert_eq!(opened.panes.len(), 2);
    // The split view must be vertical (panes side-by-side).
    assert!(opened.split_controller.splitView().isVertical());
}

fn open_split_window_horizontal_propagates_to_split_view() {
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec::default(),
        PaneSpec::default(),
    ];
    let opened =
        open_split_window("test", (800.0, 600.0), false, specs, mtm);
    assert!(!opened.split_controller.splitView().isVertical());
}

// ---------------------------------------------------------------------
// 2. PaneSpec fields propagate to NSSplitViewItem
// ---------------------------------------------------------------------

fn pane_spec_applies_min_max_and_can_collapse_to_nssplitviewitem() {
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec {
            behavior: PaneBehavior::Default,
            minimum_thickness: Some(150.0),
            maximum_thickness: Some(450.0),
            can_collapse: Some(true),
            holding_priority: Some(199.0),
            ..Default::default()
        },
    ];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);
    let item = &opened.panes[0].item;
    assert_eq!(item.minimumThickness(), 150.0);
    assert_eq!(item.maximumThickness(), 450.0);
    assert_eq!(item.canCollapse(), true);
    // NSLayoutPriority is `c_float`. Compare via f32 equality.
    let prio: f32 = unsafe { msg_send![&**item, holdingPriority] };
    assert!((prio - 199.0).abs() < 0.001);
}

fn pane_spec_initial_collapsed_true_is_collapsed() {
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec { behavior: PaneBehavior::Default, ..Default::default() },
        PaneSpec {
            behavior: PaneBehavior::Inspector,
            collapsed: true,
            ..Default::default()
        },
    ];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);
    assert_eq!(opened.is_pane_collapsed(0), false);
    assert_eq!(opened.is_pane_collapsed(1), true);
}

// ---------------------------------------------------------------------
// 3. set_pane_collapsed / is_pane_collapsed round-trip
// ---------------------------------------------------------------------

fn set_pane_collapsed_round_trips() {
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec::default(),
        PaneSpec {
            behavior: PaneBehavior::Inspector,
            can_collapse: Some(true),
            ..Default::default()
        },
    ];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);

    // Animator-driven setCollapsed in a test context (no run loop
    // animating) still flips `isCollapsed` synchronously.
    opened.set_pane_collapsed(1, true);
    assert!(opened.is_pane_collapsed(1));

    opened.set_pane_collapsed(1, false);
    assert!(!opened.is_pane_collapsed(1));
}

fn set_pane_collapsed_on_out_of_range_index_is_noop() {
    let mtm = common::test_mtm();
    let specs = vec![PaneSpec::default()];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);
    // No panic, no observable effect.
    opened.set_pane_collapsed(99, true);
    assert!(!opened.is_pane_collapsed(99));
}

// ---------------------------------------------------------------------
// 4. toggle_sidebar / toggle_inspector don't panic on any macOS
// ---------------------------------------------------------------------

fn toggle_sidebar_and_toggle_inspector_are_safe_to_call() {
    // Even on macOS < 14 (no toggleInspector:) or < 11 (no
    // toggleSidebar:), the respondsToSelector: guard makes these
    // no-ops. We just check we don't panic — the actual collapse
    // behavior is OS-version-dependent.
    let mtm = common::test_mtm();
    let specs = vec![
        PaneSpec { behavior: PaneBehavior::Sidebar, ..Default::default() },
        PaneSpec::default(),
        PaneSpec {
            behavior: PaneBehavior::Inspector,
            ..Default::default()
        },
    ];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);
    opened.toggle_sidebar();
    opened.toggle_inspector();
}

// ---------------------------------------------------------------------
// 5. viewDidLayout zero-size guard
// ---------------------------------------------------------------------

fn view_did_layout_at_zero_size_is_safe() {
    // When a pane is collapsed, AppKit fires viewDidLayout with
    // size 0×0. Running Taffy at that size would cache zero-sized
    // frames; the guard short-circuits. We can't easily detect
    // "didn't run" — but we CAN verify no panic and no exception
    // by invoking the method on a freshly-built controller whose
    // root view has zero frame.
    let mtm = common::test_mtm();
    let specs = vec![PaneSpec::default()];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);

    let controller = &opened.panes[0].controller;
    // The fresh controller's view doesn't have a frame yet
    // because the window hasn't been displayed. viewDidLayout
    // would run the zero-size guard.
    unsafe {
        let _: () = msg_send![&**controller, viewDidLayout];
    }
}

// ---------------------------------------------------------------------
// 6. Children mount under pane.root → become NSView subviews
// ---------------------------------------------------------------------

fn pane_root_receives_mounted_subviews() {
    let mtm = common::test_mtm();
    let specs = vec![PaneSpec::default()];
    let opened =
        open_split_window("test", (800.0, 600.0), true, specs, mtm);

    with_reactive_scope(|| {
        let pane = &opened.panes[0];
        // The fresh pane's NSView has no Leptos-mounted subviews.
        let before = pane.root.ns_view().subviews().len();

        let view = label().text("hello");
        let mut state = view.build();
        state.mount(pane.root, None);

        let after = pane.root.ns_view().subviews().len();
        assert_eq!(after, before + 1, "label should attach as one subview");
        std::mem::forget(state);
    });
}

// ---------------------------------------------------------------------
// 7. SplitView::build_and_install — leptos-side builder path
// ---------------------------------------------------------------------

fn split_view_build_and_install_creates_panes_in_order() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let sv = split_view()
            .vertical(true)
            .child(
                split_pane()
                    .behavior(PaneBehavior::Default)
                    .child(label().text("main")),
            )
            .child((
                split_pane()
                    .behavior(PaneBehavior::Default)
                    .child(label().text("main")),
                split_pane()
                    .behavior(PaneBehavior::Inspector)
                    .preferred_thickness(280.0)
                    .child(label().text("inspector")),
            ));

        // Path A: bare SplitView, IntoSplitView identity.
        let _sv2 = sv.into_split_view();
    });

    // Build a fresh one and actually install it.
    with_reactive_scope(|| {
        let sv = split_view()
            .vertical(true)
            .child((
                split_pane()
                    .behavior(PaneBehavior::Default)
                    .child(label().text("main")),
                split_pane()
                    .behavior(PaneBehavior::Inspector)
                    .preferred_thickness(280.0)
                    .minimum_thickness(200.0)
                    .child(label().text("inspector")),
            ));
        let (opened, state) =
            sv.build_and_install("split-test", (900.0, 600.0), mtm);

        assert_eq!(opened.panes.len(), 2);
        assert!(opened.split_controller.splitView().isVertical());
        assert_eq!(opened.panes[1].item.minimumThickness(), 200.0);

        // Each pane's NSView should have at least one Leptos
        // subview after mount_into.
        assert!(opened.panes[0].root.ns_view().subviews().len() >= 1);
        assert!(opened.panes[1].root.ns_view().subviews().len() >= 1);

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 8. Reactive `collapsed` signal flips the pane
// ---------------------------------------------------------------------

fn reactive_collapsed_signal_drives_pane_collapse() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let hidden = RwSignal::new(false);

        let sv = split_view()
            .vertical(true)
            .child((
                split_pane()
                    .behavior(PaneBehavior::Default)
                    .child(label().text("main")),
                split_pane()
                    .behavior(PaneBehavior::Inspector)
                    .preferred_thickness(280.0)
                    .can_collapse(true)
                    .collapsed(move || hidden.get())
                    .child(label().text("inspector")),
            ));
        let (opened, state) =
            sv.build_and_install("split-reactive", (900.0, 600.0), mtm);

        // Initially expanded.
        assert!(!opened.is_pane_collapsed(1));

        // Flip the signal. The `install`-wrapped effect re-runs
        // on the main queue; pump to drain it.
        hidden.set(true);
        common::pump_runloop_once();
        assert!(opened.is_pane_collapsed(1));

        // And back.
        hidden.set(false);
        common::pump_runloop_once();
        assert!(!opened.is_pane_collapsed(1));

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 9. Initial collapsed=true via reactive — sampled at to_spec via
//    untrack, so the pane comes up collapsed without first showing
//    expanded and re-animating closed.
// ---------------------------------------------------------------------

fn initial_reactive_collapsed_true_starts_collapsed() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let hidden = RwSignal::new(true);
        let sv = split_view()
            .vertical(true)
            .child((
                split_pane().child(label().text("main")),
                split_pane()
                    .behavior(PaneBehavior::Inspector)
                    .preferred_thickness(280.0)
                    .can_collapse(true)
                    .collapsed(move || hidden.get())
                    .child(label().text("inspector")),
            ));
        let (opened, state) =
            sv.build_and_install("split-initial", (900.0, 600.0), mtm);

        // Sampled-at-build initial value should be true.
        assert!(
            opened.is_pane_collapsed(1),
            "reactive collapsed=true should be sampled into PaneSpec.collapsed"
        );

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 10. IntoSplitView accepts both SplitView<P> and the wrapped form
// ---------------------------------------------------------------------

fn into_split_view_identity_for_bare_split_view() {
    let sv = split_view().child(
        split_pane().child(label().text("x")),
    );
    let _: leptos_cocoa::cocoa::split::SplitView<_> = sv.into_split_view();
}

// ---------------------------------------------------------------------
// 11. SplitPaneList::pane_count for the empty case
// ---------------------------------------------------------------------

fn split_pane_list_empty_yields_zero_panes_through_build() {
    // Routed via the `SplitPaneList for ()` impl. Empty
    // SplitView still opens a window, just with no panes.
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let sv = split_view(); // SplitView<()>
        let (opened, _state) =
            sv.build_and_install("split-empty", (600.0, 400.0), mtm);
        assert_eq!(opened.panes.len(), 0);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// main
// ---------------------------------------------------------------------

// Removed: `pages_main_pane_toolbar_keeps_height` tested the old
// hstack-shaped `toolbar()` builder. The current `<toolbar>` is
// backed by NSToolbar and lives outside the layout tree.

fn main() {
    common::run_tests(&[
        ("open_split_window_with_two_panes_yields_two_panes",
            open_split_window_with_two_panes_yields_two_panes),
        ("open_split_window_horizontal_propagates_to_split_view",
            open_split_window_horizontal_propagates_to_split_view),
        ("pane_spec_applies_min_max_and_can_collapse_to_nssplitviewitem",
            pane_spec_applies_min_max_and_can_collapse_to_nssplitviewitem),
        ("pane_spec_initial_collapsed_true_is_collapsed",
            pane_spec_initial_collapsed_true_is_collapsed),
        ("set_pane_collapsed_round_trips", set_pane_collapsed_round_trips),
        ("set_pane_collapsed_on_out_of_range_index_is_noop",
            set_pane_collapsed_on_out_of_range_index_is_noop),
        ("toggle_sidebar_and_toggle_inspector_are_safe_to_call",
            toggle_sidebar_and_toggle_inspector_are_safe_to_call),
        ("view_did_layout_at_zero_size_is_safe",
            view_did_layout_at_zero_size_is_safe),
        ("pane_root_receives_mounted_subviews",
            pane_root_receives_mounted_subviews),
        ("split_view_build_and_install_creates_panes_in_order",
            split_view_build_and_install_creates_panes_in_order),
        ("reactive_collapsed_signal_drives_pane_collapse",
            reactive_collapsed_signal_drives_pane_collapse),
        ("initial_reactive_collapsed_true_starts_collapsed",
            initial_reactive_collapsed_true_starts_collapsed),
        ("into_split_view_identity_for_bare_split_view",
            into_split_view_identity_for_bare_split_view),
        ("split_pane_list_empty_yields_zero_panes_through_build",
            split_pane_list_empty_yields_zero_panes_through_build),
    ]);
}
