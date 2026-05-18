//! Grid layout tests — build small grids, exercise the cocoa_dom grid
//! setters, run `compute_layout`, assert frames.
//!
//! Mirrors `tests/layout.rs` in style and helpers. Each test is a
//! plain `fn()` registered in `main()` so the custom harness can run
//! them on the actual main thread (AppKit is main-thread-only).

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{layout, Element};
use renderer::{auto, fr, length, GridAutoFlow};
use objc2_foundation::NSSize;
use renderer::attrs::GridLine;

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn fresh_tree(root: &Element) -> layout::TreeRef {
    let tree = layout::new_tree();
    layout::register_in_tree(root.as_node(), &tree);
    tree
}

fn frame_eq(view: &objc2_app_kit::NSView, x: f64, y: f64, w: f64, h: f64) {
    let f = view.frame();
    let tol = 0.5;
    assert!(
        (f.origin.x - x).abs() < tol
            && (f.origin.y - y).abs() < tol
            && (f.size.width - w).abs() < tol
            && (f.size.height - h).abs() < tol,
        "frame mismatch: got ({}, {}, {}×{}); expected ({}, {}, {}×{})",
        f.origin.x, f.origin.y, f.size.width, f.size.height,
        x, y, w, h
    );
}

/// Builds a grid container with the given column / row track lists,
/// no children attached yet.
fn make_grid(
    columns: Vec<renderer::GridTemplateComponent>,
    rows: Vec<renderer::GridTemplateComponent>,
) -> Element {
    let g = Element::create("grid");
    layout::set_grid_template_columns(g.as_node(), columns);
    layout::set_grid_template_rows(g.as_node(), rows);
    g
}

// =====================================================================
// 1. `<grid>` tag creates a Display::Grid container
// =====================================================================

fn create_grid_sets_display_grid() {
    let _mtm = common::test_mtm();
    let g = Element::create("grid");
    let tree = fresh_tree(&g);
    let id = g.as_node().tree_id().unwrap().1;
    let style = tree.style(id).expect("registered node has a style");
    assert_eq!(style.display, renderer::Display::Grid);
}

// =====================================================================
// 2. Template tracks lay children out in the expected cells
// =====================================================================

/// 3-column fixed-width grid: [100px, 200px, 100px]. Two auto-placed
/// children land in cols 1 and 2.
fn three_column_fixed_widths() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(100.0), length(200.0), length(100.0)],
        vec![length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 200.0, 50.0);
}

/// `fr` track sizing distributes leftover space proportionally.
/// `[1fr, 2fr, 1fr]` in a 400px grid → cols `100, 200, 100`.
fn fr_columns_distribute_leftover() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![fr(1.0), fr(2.0), fr(1.0)],
        vec![length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 200.0, 50.0);
    frame_eq(c.ns_view(), 300.0, 0.0, 100.0, 50.0);
}

/// Mixed track types: `[100px, 1fr, auto]`. With an empty content
/// `<view>` in col 3 (auto sizes to 0), col 2's fr gets all leftover.
fn mixed_fixed_fr_auto_columns() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(100.0), fr(1.0), auto()],
        vec![length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(400.0, 50.0));

    // a fixed 100, c auto = 0, b fr = leftover 300.
    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 300.0, 50.0);
    frame_eq(c.ns_view(), 400.0, 0.0, 0.0, 50.0);
}

/// 2×2 grid of fixed cells, four auto-placed children.
fn two_by_two_fills_in_row_order() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    let d = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);
    g.insert_node(d.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(100.0, 100.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 50.0, 50.0);
    frame_eq(b.ns_view(), 50.0, 0.0, 50.0, 50.0);
    frame_eq(c.ns_view(), 0.0, 50.0, 50.0, 50.0);
    frame_eq(d.ns_view(), 50.0, 50.0, 50.0, 50.0);
}

// =====================================================================
// 3. Gap separates cells (both shorthand and per-axis)
// =====================================================================

/// `set_gap(10)` puts 10px between every cell on both axes.
fn gap_shorthand_separates_both_axes() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_gap(g.as_node(), 10.0);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(110.0, 110.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 50.0, 50.0);
    frame_eq(b.ns_view(), 60.0, 0.0, 50.0, 50.0);  // 50 + 10 gap
    frame_eq(c.ns_view(), 0.0, 60.0, 50.0, 50.0);  // row gap 10
}

/// Per-axis gaps: column 5, row 20. Cells get different gaps per axis.
fn per_axis_gaps_apply_independently() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_column_gap(g.as_node(), 5.0);
    layout::set_row_gap(g.as_node(), 20.0);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(105.0, 120.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 50.0, 50.0);
    frame_eq(b.ns_view(), 55.0, 0.0, 50.0, 50.0);  // 50 + 5 col gap
    frame_eq(c.ns_view(), 0.0, 70.0, 50.0, 50.0);  // 50 + 20 row gap
}

// =====================================================================
// 4. Spanning placements
// =====================================================================

/// `grid_column.end = Span(2)` makes a child span 2 columns.
fn column_span_two_widens_cell() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let _tree = fresh_tree(&g);

    let wide = Element::create("view");
    layout::set_grid_column_end(wide.as_node(), GridLine::Span(2));
    g.insert_node(wide.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 40.0));

    // Span 2 → covers cols 1 and 2, width = 100.
    frame_eq(wide.ns_view(), 0.0, 0.0, 100.0, 40.0);
}

/// Explicit line range: `grid_column.start = Line(1), end = Line(4)`
/// spans the full 3-col grid.
fn column_range_one_to_negative_one_spans_full_width() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let _tree = fresh_tree(&g);

    let full = Element::create("view");
    layout::set_grid_column_start(full.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(full.as_node(), GridLine::Line(-1));
    g.insert_node(full.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 40.0));

    frame_eq(full.ns_view(), 0.0, 0.0, 150.0, 40.0);
}

/// Combined column + row span — a 2x2 cell in the top-left of a 3x3
/// grid of 50px cells.
fn block_spanning_two_rows_two_columns() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0), length(50.0)],
        vec![length(50.0), length(50.0), length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let block = Element::create("view");
    layout::set_grid_column_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(block.as_node(), GridLine::Line(3));
    layout::set_grid_row_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_row_end(block.as_node(), GridLine::Line(3));
    g.insert_node(block.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(150.0, 150.0));

    // Block covers (col 1..3) × (row 1..3) = 100×100.
    frame_eq(block.ns_view(), 0.0, 0.0, 100.0, 100.0);
}

// =====================================================================
// 5. `grid_line_to_placement` round-trip
// =====================================================================

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

// =====================================================================
// 6. auto_flow Row vs Column
// =====================================================================

/// With a single-column grid and `auto_flow = Row` (default), children
/// stack vertically. (Same direction makes the test trivially pass
/// even without the setter — included as a baseline.)
fn auto_flow_row_with_one_column_stacks_vertically() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(100.0)],
        vec![length(50.0), length(50.0)],
    );
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(100.0, 100.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 0.0, 50.0, 100.0, 50.0);
}

/// With a single-row grid and `auto_flow = Column`, children stack
/// horizontally instead of overflowing into implicit rows.
fn auto_flow_column_with_one_row_stacks_horizontally() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    layout::set_grid_auto_flow(g.as_node(), GridAutoFlow::Column);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    let b = Element::create("view");
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(100.0, 40.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 50.0, 40.0);
    frame_eq(b.ns_view(), 50.0, 0.0, 50.0, 40.0);
}

// =====================================================================
// 7. Empty grid + zero-size — no panic
// =====================================================================

fn empty_grid_no_panic() {
    let _mtm = common::test_mtm();
    let g = Element::create("grid");
    let _tree = fresh_tree(&g);
    layout::compute_layout(g.as_node(), NSSize::new(100.0, 100.0));
}

fn zero_available_size_no_panic() {
    let _mtm = common::test_mtm();
    let g = make_grid(vec![fr(1.0), fr(1.0)], vec![fr(1.0)]);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(0.0, 0.0));
    // No frame assertion — just verify we didn't panic.
}

// =====================================================================
// 8. Padding on the grid container insets the cells
// =====================================================================

fn padding_insets_grid_cells() {
    let _mtm = common::test_mtm();
    let g = make_grid(
        vec![length(50.0), length(50.0)],
        vec![length(50.0)],
    );
    layout::set_padding(g.as_node(), 10.0);
    let _tree = fresh_tree(&g);

    let a = Element::create("view");
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), NSSize::new(120.0, 70.0));

    // Padding 10 on all sides → first cell origin (10, 10).
    frame_eq(a.ns_view(), 10.0, 10.0, 50.0, 50.0);
}

// =====================================================================
// 9. Sibling regression — flexbox still works after grid setters land
// =====================================================================
//
// A defensive smoke test: build a non-grid flexbox stack right after
// touching the grid module, verify nothing in the shared
// renderer dedup broke the flex path. Catches accidental
// cross-pollination (e.g. set_gap now writing grid-only fields).

fn flexbox_still_works_after_grid() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    layout::set_gap(root.as_node(), 10.0);
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_width(a.as_node(), 40.0);
    layout::set_height(a.as_node(), 30.0);
    layout::set_width(b.as_node(), 40.0);
    layout::set_height(b.as_node(), 30.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), NSSize::new(200.0, 30.0));

    frame_eq(a.ns_view(), 0.0, 0.0, 40.0, 30.0);
    frame_eq(b.ns_view(), 50.0, 0.0, 40.0, 30.0);  // 40 + 10 gap
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
            "auto_flow_row_with_one_column_stacks_vertically",
            auto_flow_row_with_one_column_stacks_vertically,
        ),
        (
            "auto_flow_column_with_one_row_stacks_horizontally",
            auto_flow_column_with_one_row_stacks_horizontally,
        ),
        ("empty_grid_no_panic", empty_grid_no_panic),
        ("zero_available_size_no_panic", zero_available_size_no_panic),
        ("padding_insets_grid_cells", padding_insets_grid_cells),
        ("flexbox_still_works_after_grid", flexbox_still_works_after_grid),
    ]);
}
