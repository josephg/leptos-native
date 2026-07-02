//! Grid layout tests — GTK port. Builds small grids via the
//! gtk_dom grid setters, runs `compute_layout`, reads frames off
//! the Taffy tree, asserts.
//!
//! Same coverage as `cocoa_dom/tests/grid_layout.rs`; we read frames
//! out of the tree rather than off GTK widgets because GTK's
//! measure/allocate cycle isn't exercised in a headless test.

#![cfg(feature = "gtk")]

mod common;

use leptos_native::renderer;
use leptos_gtk::dom::GtkMakeView;
use leptos_gtk::dom::{layout::{self, GtkBackend}, GtkElem};
use leptos_native::renderer::{auto, fr, length, GridAutoFlow, Backend};
use leptos_native::renderer::attrs::GridLine;

fn frame_eq(
    el: &GtkElem,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let l = GtkBackend::layout(el.id()).expect("no layout computed");
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
) -> GtkElem {
    let g = GtkElem::create_grid();
    layout::set_grid_template_columns(g, columns);
    layout::set_grid_template_rows(g, rows);
    g
}

// ---------------------------------------------------------------------

fn create_grid_sets_display_grid() {
    let g = GtkElem::create_grid();
    let id = g.id();
    let style = GtkBackend::style(id).expect("registered node has a style");
    assert_eq!(style.display, renderer::Display::Grid);
}

fn three_column_fixed_widths() {
    let g = make_grid(vec![length(100.0), length(200.0), length(100.0)],
        vec![length(50.0)],
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);

    layout::compute_layout(g, (400.0, 50.0));

    frame_eq(&a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&b, 100.0, 0.0, 200.0, 50.0);
}

fn fr_columns_distribute_leftover() {
    let g = make_grid(vec![fr(1.0), fr(2.0), fr(1.0)],
        vec![length(50.0)],
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);
    g.insert_node(c, None);

    layout::compute_layout(g, (400.0, 50.0));

    frame_eq(&a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&b, 100.0, 0.0, 200.0, 50.0);
    frame_eq(&c, 300.0, 0.0, 100.0, 50.0);
}

fn mixed_fixed_fr_auto_columns() {
    let g = make_grid(vec![length(100.0), fr(1.0), auto()],
        vec![length(50.0)],
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);
    g.insert_node(c, None);

    layout::compute_layout(g, (400.0, 50.0));

    frame_eq(&a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&b, 100.0, 0.0, 300.0, 50.0);
    frame_eq(&c, 400.0, 0.0, 0.0, 50.0);
}

fn two_by_two_fills_in_row_order() {
    let g = make_grid(vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    let d = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);
    g.insert_node(c, None);
    g.insert_node(d, None);

    layout::compute_layout(g, (100.0, 100.0));

    frame_eq(&a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&b, 50.0, 0.0, 50.0, 50.0);
    frame_eq(&c, 0.0, 50.0, 50.0, 50.0);
    frame_eq(&d, 50.0, 50.0, 50.0, 50.0);
}

fn gap_shorthand_separates_both_axes() {
    let g = make_grid(vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_gap(g, 10.0);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);
    g.insert_node(c, None);

    layout::compute_layout(g, (110.0, 110.0));

    frame_eq(&a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&b, 60.0, 0.0, 50.0, 50.0);
    frame_eq(&c, 0.0, 60.0, 50.0, 50.0);
}

fn per_axis_gaps_apply_independently() {
    let g = make_grid(vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_column_gap(g, 5.0);
    layout::set_row_gap(g, 20.0);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);
    g.insert_node(c, None);

    layout::compute_layout(g, (105.0, 120.0));

    frame_eq(&a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&b, 55.0, 0.0, 50.0, 50.0);
    frame_eq(&c, 0.0, 70.0, 50.0, 50.0);
}

fn column_span_two_widens_cell() {
    let g = make_grid(vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );

    let wide = GtkElem::create_stack();
    layout::set_grid_column_end(wide, GridLine::Span(2));
    g.insert_node(wide, None);

    layout::compute_layout(g, (150.0, 40.0));

    frame_eq(&wide, 0.0, 0.0, 100.0, 40.0);
}

fn column_range_one_to_negative_one_spans_full_width() {
    let g = make_grid(vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );

    let full = GtkElem::create_stack();
    layout::set_grid_column_start(full, GridLine::Line(1));
    layout::set_grid_column_end(full, GridLine::Line(-1));
    g.insert_node(full, None);

    layout::compute_layout(g, (150.0, 40.0));

    frame_eq(&full, 0.0, 0.0, 150.0, 40.0);
}

fn block_spanning_two_rows_two_columns() {
    let g = make_grid(vec![length(50.0), length(50.0), length(50.0)],
        vec![length(50.0), length(50.0), length(50.0)],
    );

    let block = GtkElem::create_stack();
    layout::set_grid_column_start(block, GridLine::Line(1));
    layout::set_grid_column_end(block, GridLine::Line(3));
    layout::set_grid_row_start(block, GridLine::Line(1));
    layout::set_grid_row_end(block, GridLine::Line(3));
    g.insert_node(block, None);

    layout::compute_layout(g, (150.0, 150.0));

    frame_eq(&block, 0.0, 0.0, 100.0, 100.0);
}

fn grid_line_to_placement_handles_each_variant() {
    use leptos_native::renderer::GridPlacement;

    assert!(matches!(
        layout::grid_line_to_placement(GridLine::Auto),
        GridPlacement::Auto
    ));
    let p = layout::grid_line_to_placement(GridLine::Line(3));
    assert!(matches!(p, GridPlacement::Line(_)));
    let p = layout::grid_line_to_placement(GridLine::Span(5));
    assert!(matches!(p, GridPlacement::Span(5)));
}

fn auto_flow_row_with_one_column_stacks_vertically() {
    let g = make_grid(vec![length(100.0)],
        vec![length(50.0), length(50.0)],
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);

    layout::compute_layout(g, (100.0, 100.0));

    frame_eq(&a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&b, 0.0, 50.0, 100.0, 50.0);
}

fn auto_flow_column_with_one_row_stacks_horizontally() {
    let g = make_grid(vec![length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    layout::set_grid_auto_flow(g, GridAutoFlow::Column);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    g.insert_node(a, None);
    g.insert_node(b, None);

    layout::compute_layout(g, (100.0, 40.0));

    frame_eq(&a, 0.0, 0.0, 50.0, 40.0);
    frame_eq(&b, 50.0, 0.0, 50.0, 40.0);
}

fn empty_grid_no_panic() {
    let g = GtkElem::create_grid();
    layout::compute_layout(g, (100.0, 100.0));
}

fn zero_available_size_no_panic() {
    let g = make_grid(vec![fr(1.0), fr(1.0)], vec![fr(1.0)]);

    let a = GtkElem::create_stack();
    g.insert_node(a, None);

    layout::compute_layout(g, (0.0, 0.0));
}

fn padding_insets_grid_cells() {
    let g = make_grid(vec![length(50.0), length(50.0)],
        vec![length(50.0)],
    );
    layout::set_padding(g, 10.0);

    let a = GtkElem::create_stack();
    g.insert_node(a, None);

    layout::compute_layout(g, (120.0, 70.0));

    frame_eq(&a, 10.0, 10.0, 50.0, 50.0);
}

fn flexbox_still_works_after_grid() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);
    layout::set_gap(root, 10.0);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_width(a, 40.0);
    layout::set_height(a, 30.0);
    layout::set_width(b, 40.0);
    layout::set_height(b, 30.0);
    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (200.0, 30.0));

    frame_eq(&a, 0.0, 0.0, 40.0, 30.0);
    frame_eq(&b, 50.0, 0.0, 40.0, 30.0);
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
