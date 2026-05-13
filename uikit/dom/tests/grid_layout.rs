//! Grid layout tests — iOS port. Mirrors
//! `cocoa/dom/tests/grid_layout.rs` and `gtk/dom/tests/grid_layout.rs`.
//!
//! Runs on the iOS simulator via:
//!
//!   cargo test -p ios_dom --target aarch64-apple-ios-sim --test grid_layout
//!
//! Requires a booted simulator (`xcrun simctl boot ...`). The cargo
//! runner for `aarch64-apple-ios-sim` is configured in
//! `uikit/dom/.cargo/config.toml`.

#![cfg(target_os = "ios")]

mod common;

use ios_dom::{layout, Element};
use renderer::{auto, fr, length, GridAutoFlow};
use objc2_foundation::NSSize;
use renderer::attrs::GridLine;

fn fresh_tree(root: &Element) -> layout::TreeRef {
    let tree = layout::new_tree();
    layout::register_in_tree(root.as_node(), &tree);
    tree
}

/// Read the Taffy-computed layout for `el` and assert position +
/// size. Reads from the tree rather than via `UIView::frame()`
/// because tests don't run a UIKit event loop, so the iOS-side
/// `apply_layout` walk (which sets frames) is the bit under test.
fn frame_eq(
    tree: &layout::TreeRef,
    el: &Element,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let lh = el
        .as_node()
        .layout_slot()
        .borrow()
        .handle
        .clone()
        .expect("element not registered in tree");
    let l = tree.layout(lh.node_id).expect("no layout computed");
    let tol = 0.5;
    assert!(
        (l.location.x - x).abs() < tol
            && (l.location.y - y).abs() < tol
            && (l.size.width - w).abs() < tol
            && (l.size.height - h).abs() < tol,
        "frame mismatch: got ({}, {}, {}×{}); expected ({}, {}, {}×{})",
        l.location.x, l.location.y, l.size.width, l.size.height,
        x, y, w, h
    );
}

fn make_grid(
    columns: Vec<renderer::GridTemplateComponent>,
    rows: Vec<renderer::GridTemplateComponent>,
) -> Element {
    let _mtm = common::test_mtm();
    let g = Element::create("grid");
    layout::set_grid_template_columns(g.as_node(), columns);
    layout::set_grid_template_rows(g.as_node(), rows);
    g
}

// ---------------------------------------------------------------------

fn create_grid_sets_display_grid() {
    let _mtm = common::test_mtm();
    let g = Element::create("grid");
    let tree = fresh_tree(&g);
    let id = g.as_node().layout_slot().borrow().handle.as_ref().unwrap().node_id;
    let style = tree.style(id).expect("registered node has a style");
    assert_eq!(style.display, renderer::Display::Grid);
}

fn three_column_fixed_widths() {
    let g = make_grid(
        vec![length(100.0), length(200.0), length(100.0)],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 200.0, 50.0);
}

fn fr_columns_distribute_leftover() {
    let g = make_grid(
        vec![fr(1.0), fr(2.0), fr(1.0)],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 200.0, 50.0);
    frame_eq(&tree, &c, 300.0, 0.0, 100.0, 50.0);
}

fn mixed_fixed_fr_auto_columns() {
    let g = make_grid(
        vec![length(100.0), fr(1.0), auto()],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 300.0, 50.0);
    frame_eq(&tree, &c, 400.0, 0.0, 0.0, 50.0);
}

fn two_by_two_fills_in_row_order() {
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    let d = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);
    g.insert_node(d.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(100.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 50.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 50.0, 50.0, 50.0);
    frame_eq(&tree, &d, 50.0, 50.0, 50.0, 50.0);
}

fn gap_shorthand_separates_both_axes() {
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_gap(g.as_node(), 10.0);
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(110.0, 110.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 60.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 60.0, 50.0, 50.0);
}

fn per_axis_gaps_apply_independently() {
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_column_gap(g.as_node(), 5.0);
    layout::set_row_gap(g.as_node(), 20.0);
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(105.0, 120.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 55.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 70.0, 50.0, 50.0);
}

fn column_span_two_widens_cell() {
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let tree = fresh_tree(&g);

    let wide = Element::create("view");
    layout::set_grid_column_end(wide.as_node(), GridLine::Span(2));
    g.insert_node(wide.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 40.0));

    frame_eq(&tree, &wide, 0.0, 0.0, 100.0, 40.0);
}

fn column_range_one_to_negative_one_spans_full_width() {
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let tree = fresh_tree(&g);

    let full = Element::create("view");
    layout::set_grid_column_start(full.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(full.as_node(), GridLine::Line(-1));
    g.insert_node(full.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 40.0));

    frame_eq(&tree, &full, 0.0, 0.0, 150.0, 40.0);
}

fn block_spanning_two_rows_two_columns() {
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(50.0), length(50.0), length(50.0)],
    );
    let tree = fresh_tree(&g);

    let block = Element::create("view");
    layout::set_grid_column_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(block.as_node(), GridLine::Line(3));
    layout::set_grid_row_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_row_end(block.as_node(), GridLine::Line(3));
    g.insert_node(block.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 150.0));

    frame_eq(&tree, &block, 0.0, 0.0, 100.0, 100.0);
}

fn grid_line_to_placement_handles_each_variant() {
    use renderer::GridPlacement;
    let _mtm = common::test_mtm();

    assert!(matches!(
        layout::grid_line_to_placement(GridLine::Auto),
        GridPlacement::Auto
    ));
    let p = layout::grid_line_to_placement(GridLine::Line(3));
    assert!(matches!(p, GridPlacement::Line(_)));
    let p = layout::grid_line_to_placement(GridLine::Span(5));
    assert!(matches!(p, GridPlacement::Span(5)));
}

fn auto_flow_column_with_one_row_stacks_horizontally() {
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    layout::set_grid_auto_flow(g.as_node(), GridAutoFlow::Column);
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(100.0, 40.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 40.0);
    frame_eq(&tree, &b, 50.0, 0.0, 50.0, 40.0);
}

fn padding_insets_grid_cells() {
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0)],
    );
    layout::set_padding(g.as_node(), 10.0);
    let tree = fresh_tree(&g);

    let a = Element::create("view");
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(120.0, 70.0));

    frame_eq(&tree, &a, 10.0, 10.0, 50.0, 50.0);
}

fn empty_grid_no_panic() {
    let _mtm = common::test_mtm();
    let g = Element::create("grid");
    let _tree = fresh_tree(&g);
    layout::compute_layout(g.as_node(), NSSize::new(100.0, 100.0));
}

fn zero_available_size_no_panic() {
    let g = make_grid(vec![fr(1.0), fr(1.0)], vec![fr(1.0)]);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(0.0, 0.0));
}

fn main() {
    common::run_tests(&[
        ("create_grid_sets_display_grid", create_grid_sets_display_grid),
        ("three_column_fixed_widths", three_column_fixed_widths),
        ("fr_columns_distribute_leftover", fr_columns_distribute_leftover),
        ("mixed_fixed_fr_auto_columns", mixed_fixed_fr_auto_columns),
        ("two_by_two_fills_in_row_order", two_by_two_fills_in_row_order),
        ("gap_shorthand_separates_both_axes", gap_shorthand_separates_both_axes),
        ("per_axis_gaps_apply_independently", per_axis_gaps_apply_independently),
        ("column_span_two_widens_cell", column_span_two_widens_cell),
        (
            "column_range_one_to_negative_one_spans_full_width",
            column_range_one_to_negative_one_spans_full_width,
        ),
        (
            "block_spanning_two_rows_two_columns",
            block_spanning_two_rows_two_columns,
        ),
        (
            "grid_line_to_placement_handles_each_variant",
            grid_line_to_placement_handles_each_variant,
        ),
        (
            "auto_flow_column_with_one_row_stacks_horizontally",
            auto_flow_column_with_one_row_stacks_horizontally,
        ),
        ("padding_insets_grid_cells", padding_insets_grid_cells),
        ("empty_grid_no_panic", empty_grid_no_panic),
        ("zero_available_size_no_panic", zero_available_size_no_panic),
    ]);
}
