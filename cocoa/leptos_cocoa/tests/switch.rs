//! `<Switch>` / `<Match>` control-flow tests.
//!
//! Exercises the cocoa-driven Render pipeline so the test
//! double-checks both the trait logic (which arm wins) AND the
//! rebuild path (when a signal flips, the right view ends up
//! mounted). Uses direct builder calls + the leptos `Switch` /
//! `Match` components (no `view!{}` macro — the `::leptos::tachys`
//! path it emits doesn't resolve inside the `leptos_cocoa` crate
//! itself).

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::prelude::*;
use leptos_cocoa::cocoa::element::{label, vstack};
use leptos::children::ToChildren;
use reactive_graph::owner::Owner;
use reactive_graph::signal::RwSignal;
use reactive_graph::traits::{Get, Set};
use renderer::view::{Mountable, Render};

fn with_reactive_scope<F: FnOnce()>(f: F) {
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

/// Build a Match arm with the given predicate + label-text child.
/// Wraps the verbose `Match(MatchProps::builder()...)` call.
fn arm(
    when: impl Fn() -> bool + Send + 'static,
    text: &'static str,
) -> leptos::control_flow::Match<leptos_cocoa::cocoa::element::Label, Dom> {
    leptos::control_flow::Match(
        leptos::control_flow::MatchProps::builder()
            .when(when)
            .children(ToChildren::to_children(move || label().text(text)))
            .build()
    )
}

/// Mount a renderable view inside a host vstack so we can inspect
/// what actually got mounted as direct subviews. Returns the host
/// and the View::State (kept alive for the test's lifetime).
fn mount_into_host<V: Render<Dom> + 'static>(
    view: V,
) -> (cocoa_dom::Element, <V as Render<Dom>>::State) {
    let tree = cocoa_dom::layout::new_tree();
    let host = cocoa_dom::Element::create_container_with(
        &tree,
        common::test_mtm(),
    );
    cocoa_dom::layout::set_as_root(host.as_node(), &tree);
    let mut state = view.build(&tree);
    state.mount(&host, None);
    (host, state)
}

/// Count the *visible* (non-hidden) direct subviews of `host`.
/// `<Switch>` always has at least one direct subview — a hidden
/// placeholder that serves as the mount anchor for None→Some
/// transitions. Tests care about user-visible content, so filter
/// hidden views.
fn count_visible_subviews(host: &objc2_app_kit::NSView) -> usize {
    host.subviews()
        .iter()
        .filter(|sv| !sv.isHidden())
        .count()
}

/// Read the label-text of the first visible NSTextField subview.
/// Skips the Switch placeholder (which is hidden) and any other
/// hidden views.
fn first_visible_label_text(host: &objc2_app_kit::NSView) -> String {
    for sv in host.subviews().iter() {
        if sv.isHidden() { continue; }
        if let Some(f) =
            sv.downcast_ref::<objc2_app_kit::NSTextField>()
        {
            return f.stringValue().to_string();
        }
    }
    panic!("no visible label subview found under host");
}

// ---------------------------------------------------------------------
// 1. Single Match — when=true renders the child
// ---------------------------------------------------------------------

fn single_match_when_true_renders_child() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let arms = (arm(|| true, "hello"),);
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);
        let inner = count_visible_subviews(&vstack_view);
        assert_eq!(
            inner, 1,
            "Switch with one matching <Match> should mount exactly \
             one child (the label); got {inner}",
        );
        assert_eq!(first_visible_label_text(&vstack_view), "hello");
    });
}

// ---------------------------------------------------------------------
// 2. Single Match — when=false renders nothing
// ---------------------------------------------------------------------

fn single_match_when_false_renders_nothing() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let arms = (arm(|| false, "hello"),);
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);
        let inner = count_visible_subviews(&vstack_view);
        assert_eq!(
            inner, 0,
            "Switch with no matching arm should mount nothing; \
             got {inner}",
        );
    });
}

// ---------------------------------------------------------------------
// 3. Two Matches — first-match wins
// ---------------------------------------------------------------------

fn two_matches_first_wins() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // Both arms true; FIRST should win.
        let arms = (arm(|| true, "first"), arm(|| true, "second"));
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);
        assert_eq!(count_visible_subviews(&vstack_view), 1);
        assert_eq!(
            first_visible_label_text(&vstack_view),
            "first",
            "first-match-wins: with both arms true, the FIRST <Match> \
             must be selected",
        );
    });
}

// ---------------------------------------------------------------------
// 4. Reactive transition between arms
// ---------------------------------------------------------------------

fn signal_change_swaps_active_arm() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let tab = RwSignal::new(0_u8);
        let arms = (
            arm(move || tab.get() == 0, "zero"),
            arm(move || tab.get() == 1, "one"),
        );
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);

        // Initial: tab=0 → first arm.
        assert_eq!(count_visible_subviews(&vstack_view), 1);
        assert_eq!(first_visible_label_text(&vstack_view), "zero");

        // Flip: tab=1 → second arm. RenderEffect delivers its
        // re-run via the main dispatch queue; pump it once so the
        // rebuild lands before we read.
        tab.set(1);
        common::pump_runloop_once();
        assert_eq!(count_visible_subviews(&vstack_view), 1);
        assert_eq!(
            first_visible_label_text(&vstack_view),
            "one",
            "after `tab.set(1)`, the second arm should be mounted",
        );

        // Flip back.
        tab.set(0);
        common::pump_runloop_once();
        assert_eq!(first_visible_label_text(&vstack_view), "zero");
    });
}

// ---------------------------------------------------------------------
// 5. All-false → nothing mounted
// ---------------------------------------------------------------------

fn no_match_renders_nothing_with_two_arms() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let arms = (arm(|| false, "a"), arm(|| false, "b"));
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);
        assert_eq!(count_visible_subviews(&vstack_view), 0);
    });
}

// ---------------------------------------------------------------------
// 6. Reactive flip into / out of "no match" state
// ---------------------------------------------------------------------

fn transition_through_no_match() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let sel = RwSignal::new(0_u8);
        let arms = (
            arm(move || sel.get() == 1, "one"),
            arm(move || sel.get() == 2, "two"),
        );
        let switch = leptos::control_flow::Switch(
            leptos::control_flow::SwitchProps::builder()
                .children(ToChildren::to_children(move || arms))
                .build()
        );
        let view = vstack().child(switch);
        let (host, _state) = mount_into_host(view);
        let vstack_view = host.ns_view().subviews().objectAtIndex(0);

        // sel=0 → no arm matches.
        assert_eq!(count_visible_subviews(&vstack_view), 0);

        // sel=1 → first arm mounts.
        sel.set(1);
        common::pump_runloop_once();
        assert_eq!(count_visible_subviews(&vstack_view), 1);
        assert_eq!(first_visible_label_text(&vstack_view), "one");

        // sel=2 → first unmounts, second mounts.
        sel.set(2);
        common::pump_runloop_once();
        assert_eq!(count_visible_subviews(&vstack_view), 1);
        assert_eq!(first_visible_label_text(&vstack_view), "two");

        // sel=0 → no arm: should unmount second.
        sel.set(0);
        common::pump_runloop_once();
        assert_eq!(count_visible_subviews(&vstack_view), 0);
    });
}

// ---------------------------------------------------------------------
// main
// ---------------------------------------------------------------------

fn main() {
    common::run_tests(&[
        ("single_match_when_true_renders_child",
            single_match_when_true_renders_child),
        ("single_match_when_false_renders_nothing",
            single_match_when_false_renders_nothing),
        ("two_matches_first_wins", two_matches_first_wins),
        ("signal_change_swaps_active_arm", signal_change_swaps_active_arm),
        ("no_match_renders_nothing_with_two_arms",
            no_match_renders_nothing_with_two_arms),
        ("transition_through_no_match", transition_through_no_match),
    ]);
}
