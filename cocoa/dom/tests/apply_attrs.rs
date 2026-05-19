//! Tests for `renderer::apply_layout` and `apply_universal`,
//! exercised through cocoa_dom's `Element` (which impls
//! `LayoutElement` + `UniversalElement`).
//!
//! Goals:
//! - Static `LayoutAttrs` values reach the right Taffy-style fields
//!   on the underlying node.
//! - Reactive values install a `RenderEffect` that fires the setter
//!   when the signal changes.
//! - `Dim` variants land in the correct `s.{size,min_size,max_size}.{width,height}` slot.
//! - `apply_universal` drives `set_alpha` and `set_tool_tip`.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{layout, Element};
use renderer::{Dimension, LengthPercentage};
use reactive_graph::{
    owner::Owner,
    signal::RwSignal,
    traits::{Get, Set},
};
use renderer::attrs::{
    AlignSelf, Dim, GridLine, LayoutAttrs, MaybeReactive, UniversalAttrs,
};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn fresh_tree(root: &Element) -> layout::TreeRef {
    // Element is already in a tree (eager allocation); just publish
    // it as the root if it isn't already.
    let (tree, _) = root.as_node().tree_id().expect("element has tree");
    layout::set_as_root(root.as_node(), &tree);
    tree
}

fn style_of(el: &Element) -> renderer::Style {
    el.as_node().with_style(|s| s.clone())
}

/// Wrap the test body in a fresh reactive `Owner`. `RenderEffect`s
/// take an owner from thread-local context; without one their setup
/// closure never fires and our assertions all see zeros.
fn with_owner<F: FnOnce()>(body: F) {
    let owner = Owner::new();
    owner.with(body);
}

/// Register the AppKit dispatch-queue executor once. `RenderEffect`
/// uses `Executor::spawn_local()` internally; without an executor
/// registered the first signal subscription panics. `init_app` does
/// this for the wider integration tests; reactive-only unit tests
/// install the spawner directly.
fn init_executor_once(mtm: cocoa_dom::MainThreadMarker) {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Test process — leak the returned (app, delegate)
        // intentionally; they're singletons for the test run.
        let (app, delegate) = cocoa_dom::app::init_app(mtm);
        std::mem::forget(app);
        std::mem::forget(delegate);
    });
}

// ---------------------------------------------------------------------
// 1. Static LayoutAttrs land in the right Taffy slots
// ---------------------------------------------------------------------

fn padding_static_lands_in_padding_field() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.padding = Some(MaybeReactive::Static(renderer::attrs::Edges::all(8.0)));
    let effects = layout::apply_layout(&el, attrs);
    assert!(effects.is_empty(), "static value must not retain effects");

    let s = style_of(&el);
    assert_eq!(s.padding.left, LengthPercentage::length(8.0));
    assert_eq!(s.padding.right, LengthPercentage::length(8.0));
    assert_eq!(s.padding.top, LengthPercentage::length(8.0));
    assert_eq!(s.padding.bottom, LengthPercentage::length(8.0));
}

fn flex_grow_static_lands_in_flex_grow() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.flex_grow = Some(MaybeReactive::Static(2.5));
    let _ = layout::apply_layout(&el, attrs);

    assert_eq!(style_of(&el).flex_grow, 2.5);
}

fn align_self_static_converts_to_taffy_alignitems() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.align_self = Some(MaybeReactive::Static(AlignSelf::Center));
    let _ = layout::apply_layout(&el, attrs);

    assert_eq!(style_of(&el).align_self, Some(layout::AlignItems::Center));
}

fn align_self_auto_clears_to_none() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.align_self = Some(MaybeReactive::Static(AlignSelf::Auto));
    let _ = layout::apply_layout(&el, attrs);

    assert_eq!(style_of(&el).align_self, None);
}

// ---------------------------------------------------------------------
// 2. Dim variants reach the correct s.{size,min_size,max_size} field
// ---------------------------------------------------------------------

fn width_dim_lands_in_size_width() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.width = Some(MaybeReactive::Static(Dim::Px(120.0)));
    let _ = layout::apply_layout(&el, attrs);

    let s = style_of(&el);
    assert_eq!(s.size.width, Dimension::length(120.0));
    // height untouched
    assert_eq!(s.size.height, Dimension::auto());
}

fn height_dim_pct_lands_in_size_height_as_percent() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.height = Some(MaybeReactive::Static(Dim::Pct(0.5)));
    let _ = layout::apply_layout(&el, attrs);

    assert_eq!(style_of(&el).size.height, Dimension::percent(0.5));
}

fn min_max_dim_land_in_their_slots() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.min_width = Some(MaybeReactive::Static(Dim::Px(40.0)));
    attrs.min_height = Some(MaybeReactive::Static(Dim::Px(50.0)));
    attrs.max_width = Some(MaybeReactive::Static(Dim::Px(400.0)));
    attrs.max_height = Some(MaybeReactive::Static(Dim::Px(500.0)));
    let _ = layout::apply_layout(&el, attrs);

    let s = style_of(&el);
    assert_eq!(s.min_size.width, Dimension::length(40.0));
    assert_eq!(s.min_size.height, Dimension::length(50.0));
    assert_eq!(s.max_size.width, Dimension::length(400.0));
    assert_eq!(s.max_size.height, Dimension::length(500.0));
}

// ---------------------------------------------------------------------
// 3. Grid placement attrs convert via grid_line_to_placement
// ---------------------------------------------------------------------

fn grid_column_start_static_lands_as_line() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.grid_column_start = Some(MaybeReactive::Static(GridLine::Line(2)));
    let _ = layout::apply_layout(&el, attrs);

    let s = style_of(&el);
    assert!(matches!(s.grid_column.start, renderer::GridPlacement::Line(_)));
    assert!(matches!(s.grid_column.end, renderer::GridPlacement::Auto));
}

fn grid_row_end_static_span() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = LayoutAttrs::default();
    attrs.grid_row_end = Some(MaybeReactive::Static(GridLine::Span(3)));
    let _ = layout::apply_layout(&el, attrs);

    assert_eq!(style_of(&el).grid_row.end, renderer::GridPlacement::Span(3));
}

// ---------------------------------------------------------------------
// 4. Reactive values install effects + re-fire on signal change
// ---------------------------------------------------------------------

fn reactive_padding_re_runs_on_signal_change() {
    let mtm = common::test_mtm();
    init_executor_once(mtm);
    with_owner(|| {
        let tree = layout::new_tree();
        let el = Element::create(&tree, "view");
        let _tree = fresh_tree(&el);

        let pad = RwSignal::new(4.0_f32);
        let mut attrs = LayoutAttrs::default();
        attrs.padding = Some(MaybeReactive::Reactive(Box::new(move || {
            renderer::attrs::Edges::all(pad.get())
        })));
        let effects = layout::apply_layout(&el, attrs);
        assert_eq!(effects.len(), 1, "reactive value must retain one effect");

        // Initial fire (synchronous on RenderEffect::new).
        assert_eq!(
            style_of(&el).padding.left,
            LengthPercentage::length(4.0)
        );

        // Update the signal, drain the run loop so the queued effect
        // fires, then re-check.
        pad.set(20.0);
        common::pump_run_loop(0.1);
        assert_eq!(
            style_of(&el).padding.left,
            LengthPercentage::length(20.0)
        );

        // Keep `effects` alive until end of body so the RenderEffect
        // doesn't drop and stop firing mid-test.
        drop(effects);
    });
}

fn reactive_width_drives_size_width() {
    let mtm = common::test_mtm();
    init_executor_once(mtm);
    with_owner(|| {
        let tree = cocoa_dom::layout::new_tree();
        let el = Element::create(&tree, "view");
        let _tree = fresh_tree(&el);

        let w = RwSignal::new(Dim::Px(50.0));
        let mut attrs = LayoutAttrs::default();
        attrs.width = Some(MaybeReactive::Reactive(Box::new(move || w.get())));
        let effects = layout::apply_layout(&el, attrs);

        assert_eq!(style_of(&el).size.width, Dimension::length(50.0));

        w.set(Dim::Pct(0.75));
        common::pump_run_loop(0.1);
        assert_eq!(style_of(&el).size.width, Dimension::percent(0.75));

        drop(effects);
    });
}

// ---------------------------------------------------------------------
// 5. Empty LayoutAttrs is a no-op
// ---------------------------------------------------------------------

fn empty_layout_attrs_returns_no_effects() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let effects = layout::apply_layout(&el, LayoutAttrs::default());
    assert!(effects.is_empty(), "no fields = no effects");

    // Style remains at default — flex_grow=0, padding=zero, etc.
    let s = style_of(&el);
    assert_eq!(s.flex_grow, 0.0);
    assert_eq!(s.padding.left, LengthPercentage::length(0.0));
}

// ---------------------------------------------------------------------
// 6. apply_universal — alpha + tool_tip
// ---------------------------------------------------------------------

fn alpha_static_sets_view_alpha() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = UniversalAttrs::default();
    attrs.alpha = Some(MaybeReactive::Static(0.5));
    let _ = layout::apply_universal(&el, attrs);

    // NSView::alphaValue is a CGFloat; tolerance for f64 compare.
    let alpha = el.ns_view().alphaValue();
    assert!((alpha - 0.5).abs() < 1e-6, "got alpha={alpha}");
}

fn tool_tip_static_sets_view_tool_tip() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let mut attrs = UniversalAttrs::default();
    attrs.tool_tip = Some(MaybeReactive::Static("hello".to_string()));
    let _ = layout::apply_universal(&el, attrs);

    let tip = el.ns_view().toolTip();
    assert_eq!(tip.as_deref().map(|s| s.to_string()), Some("hello".to_string()));
}

fn reactive_alpha_re_runs_on_signal_change() {
    let mtm = common::test_mtm();
    init_executor_once(mtm);
    with_owner(|| {
        let tree = layout::new_tree();
        let el = Element::create(&tree, "view");
        let _tree = fresh_tree(&el);

        let a = RwSignal::new(1.0_f64);
        let mut attrs = UniversalAttrs::default();
        attrs.alpha = Some(MaybeReactive::Reactive(Box::new(move || a.get())));
        let effects = layout::apply_universal(&el, attrs);

        assert!((el.ns_view().alphaValue() - 1.0).abs() < 1e-6);

        a.set(0.25);
        common::pump_run_loop(0.1);
        assert!((el.ns_view().alphaValue() - 0.25).abs() < 1e-6);

        drop(effects);
    });
}

fn empty_universal_attrs_returns_no_effects() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create(&tree, "view");
    let _tree = fresh_tree(&el);

    let effects = layout::apply_universal(&el, UniversalAttrs::default());
    assert!(effects.is_empty());
}

fn main() {
    common::run_tests(&[
        ("padding_static_lands_in_padding_field", padding_static_lands_in_padding_field),
        ("flex_grow_static_lands_in_flex_grow", flex_grow_static_lands_in_flex_grow),
        (
            "align_self_static_converts_to_taffy_alignitems",
            align_self_static_converts_to_taffy_alignitems,
        ),
        ("align_self_auto_clears_to_none", align_self_auto_clears_to_none),
        ("width_dim_lands_in_size_width", width_dim_lands_in_size_width),
        (
            "height_dim_pct_lands_in_size_height_as_percent",
            height_dim_pct_lands_in_size_height_as_percent,
        ),
        ("min_max_dim_land_in_their_slots", min_max_dim_land_in_their_slots),
        (
            "grid_column_start_static_lands_as_line",
            grid_column_start_static_lands_as_line,
        ),
        ("grid_row_end_static_span", grid_row_end_static_span),
        (
            "reactive_padding_re_runs_on_signal_change",
            reactive_padding_re_runs_on_signal_change,
        ),
        ("reactive_width_drives_size_width", reactive_width_drives_size_width),
        ("empty_layout_attrs_returns_no_effects", empty_layout_attrs_returns_no_effects),
        ("alpha_static_sets_view_alpha", alpha_static_sets_view_alpha),
        ("tool_tip_static_sets_view_tool_tip", tool_tip_static_sets_view_tool_tip),
        ("reactive_alpha_re_runs_on_signal_change", reactive_alpha_re_runs_on_signal_change),
        ("empty_universal_attrs_returns_no_effects", empty_universal_attrs_returns_no_effects),
    ]);
}
