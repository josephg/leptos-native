//! Grid layout tests — GTK port. Builds small grids via the
//! gtk_dom grid setters, runs `compute_layout`, reads frames off
//! the Taffy tree, asserts.
//!
//! Same coverage as `cocoa_dom/tests/grid_layout.rs`; we read frames
//! out of the tree rather than off GTK widgets because GTK's
//! measure/allocate cycle isn't exercised in a headless test.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{layout, Node};
use renderer::{auto, fr, length, GridAutoFlow};
use renderer::attrs::GridLine;

fn fresh_tree(root: &Node) -> layout::TreeRef {
    // Node is already in a tree (eager allocation); just publish
    // it as the root if it isn't already.
    let (tree, _) = root.as_node().tree_id().expect("element has tree");
    layout::set_as_root(root.as_node(), &tree);
    tree
}

fn frame_eq(
    tree: &layout::TreeRef,
    el: &Node,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let lh = el
        .as_node()
        .mounted_handle()
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
    tree: &layout::TreeRef,
    columns: Vec<renderer::GridTemplateComponent>,
    rows: Vec<renderer::GridTemplateComponent>,
) -> Node {
    let g = Node::create_grid(tree);
    layout::set_grid_template_columns(g.as_node(), columns);
    layout::set_grid_template_rows(g.as_node(), rows);
    g
}

// ---------------------------------------------------------------------

fn create_grid_sets_display_grid() {
    let tree = gtk_dom::layout::new_tree();
    let g = Node::create_grid(&tree);
    let tree = fresh_tree(&g);
    let id = g.as_node().tree_id().unwrap().1;
    let style = tree.style(id).expect("registered node has a style");
    assert_eq!(style.display, renderer::Display::Grid);
}

fn three_column_fixed_widths() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(100.0), length(200.0), length(100.0)],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), (400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 200.0, 50.0);
}

fn fr_columns_distribute_leftover() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![fr(1.0), fr(2.0), fr(1.0)],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    let c = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), (400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 200.0, 50.0);
    frame_eq(&tree, &c, 300.0, 0.0, 100.0, 50.0);
}

fn mixed_fixed_fr_auto_columns() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(100.0), fr(1.0), auto()],
        vec![length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    let c = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), (400.0, 50.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 300.0, 50.0);
    frame_eq(&tree, &c, 400.0, 0.0, 0.0, 50.0);
}

fn two_by_two_fills_in_row_order() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    let c = Node::create_stack(&tree);
    let d = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);
    g.insert_node(d.as_node(), None);

    layout::compute_layout(g.as_node(), (100.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 50.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 50.0, 50.0, 50.0);
    frame_eq(&tree, &d, 50.0, 50.0, 50.0, 50.0);
}

fn gap_shorthand_separates_both_axes() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_gap(g.as_node(), 10.0);
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    let c = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), (110.0, 110.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 60.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 60.0, 50.0, 50.0);
}

fn per_axis_gaps_apply_independently() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0)],
        vec![length(50.0), length(50.0)],
    );
    layout::set_column_gap(g.as_node(), 5.0);
    layout::set_row_gap(g.as_node(), 20.0);
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    let c = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);
    g.insert_node(c.as_node(), None);

    layout::compute_layout(g.as_node(), (105.0, 120.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &b, 55.0, 0.0, 50.0, 50.0);
    frame_eq(&tree, &c, 0.0, 70.0, 50.0, 50.0);
}

fn column_span_two_widens_cell() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let tree = fresh_tree(&g);

    let wide = Node::create_stack(&tree);
    layout::set_grid_column_end(wide.as_node(), GridLine::Span(2));
    g.insert_node(wide.as_node(), None);

    layout::compute_layout(g.as_node(), (150.0, 40.0));

    frame_eq(&tree, &wide, 0.0, 0.0, 100.0, 40.0);
}

fn column_range_one_to_negative_one_spans_full_width() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    let tree = fresh_tree(&g);

    let full = Node::create_stack(&tree);
    layout::set_grid_column_start(full.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(full.as_node(), GridLine::Line(-1));
    g.insert_node(full.as_node(), None);

    layout::compute_layout(g.as_node(), (150.0, 40.0));

    frame_eq(&tree, &full, 0.0, 0.0, 150.0, 40.0);
}

fn block_spanning_two_rows_two_columns() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0), length(50.0)],
        vec![length(50.0), length(50.0), length(50.0)],
    );
    let tree = fresh_tree(&g);

    let block = Node::create_stack(&tree);
    layout::set_grid_column_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_column_end(block.as_node(), GridLine::Line(3));
    layout::set_grid_row_start(block.as_node(), GridLine::Line(1));
    layout::set_grid_row_end(block.as_node(), GridLine::Line(3));
    g.insert_node(block.as_node(), None);

    layout::compute_layout(g.as_node(), (150.0, 150.0));

    frame_eq(&tree, &block, 0.0, 0.0, 100.0, 100.0);
}

fn grid_line_to_placement_handles_each_variant() {
    use renderer::GridPlacement;

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
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(100.0)],
        vec![length(50.0), length(50.0)],
    );
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), (100.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 0.0, 50.0, 100.0, 50.0);
}

fn auto_flow_column_with_one_row_stacks_horizontally() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0)],
        vec![length(40.0)],
    );
    layout::set_grid_auto_flow(g.as_node(), GridAutoFlow::Column);
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);
    g.insert_node(b.as_node(), None);

    layout::compute_layout(g.as_node(), (100.0, 40.0));

    frame_eq(&tree, &a, 0.0, 0.0, 50.0, 40.0);
    frame_eq(&tree, &b, 50.0, 0.0, 50.0, 40.0);
}

fn empty_grid_no_panic() {
    let tree = gtk_dom::layout::new_tree();
    let g = Node::create_grid(&tree);
    let _tree = fresh_tree(&g);
    layout::compute_layout(g.as_node(), (100.0, 100.0));
}

fn zero_available_size_no_panic() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![fr(1.0), fr(1.0)], vec![fr(1.0)]);
    let _tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), (0.0, 0.0));
}

fn padding_insets_grid_cells() {
    let tree = gtk_dom::layout::new_tree();
    let g = make_grid(&tree, vec![length(50.0), length(50.0)],
        vec![length(50.0)],
    );
    layout::set_padding(g.as_node(), 10.0);
    let tree = fresh_tree(&g);

    let a = Node::create_stack(&tree);
    g.insert_node(a.as_node(), None);

    layout::compute_layout(g.as_node(), (120.0, 70.0));

    frame_eq(&tree, &a, 10.0, 10.0, 50.0, 50.0);
}

fn flexbox_still_works_after_grid() {
    let tree = gtk_dom::layout::new_tree();
    let root = Node::create_stack(&tree);
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    layout::set_gap(root.as_node(), 10.0);
    let tree = fresh_tree(&root);

    let a = Node::create_stack(&tree);
    let b = Node::create_stack(&tree);
    layout::set_width(a.as_node(), 40.0);
    layout::set_height(a.as_node(), 30.0);
    layout::set_width(b.as_node(), 40.0);
    layout::set_height(b.as_node(), 30.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (200.0, 30.0));

    frame_eq(&tree, &a, 0.0, 0.0, 40.0, 30.0);
    frame_eq(&tree, &b, 50.0, 0.0, 40.0, 30.0);
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
