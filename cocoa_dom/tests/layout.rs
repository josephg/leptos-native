//! Layout tests — build small trees, set Taffy styles via the
//! cocoa_dom helpers, run `compute_layout`, assert frames.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{layout, Element};
use objc2_foundation::NSSize;

/// Build a fresh layout tree, register `root` as its root, return
/// the tree handle. Tests that want layout to actually compute need
/// this — root.compute_layout panics if the node isn't registered.
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

// ---------------------------------------------------------------------
// Single root sized to fill
// ---------------------------------------------------------------------

fn root_fills_available_space() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    let _tree = fresh_tree(&root);
    layout::compute_layout(
        root.as_node(), NSSize::new(400.0, 300.0)
    );
    frame_eq(root.ns_view(), 0.0, 0.0, 400.0, 300.0);
}

// ---------------------------------------------------------------------
// Row direction — children placed side by side
// ---------------------------------------------------------------------

fn row_two_children_side_by_side() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_width(a.as_node(), 100.0);
    layout::set_height(a.as_node(), 50.0);
    layout::set_width(b.as_node(), 200.0);
    layout::set_height(b.as_node(), 60.0);

    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(500.0, 400.0)
    );

    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 200.0, 60.0);
}

// ---------------------------------------------------------------------
// Column direction — children stacked
// ---------------------------------------------------------------------

fn column_two_children_stacked() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(), layout::FlexDirection::Column
    );
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_height(a.as_node(), 80.0);
    layout::set_height(b.as_node(), 120.0);

    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(300.0, 500.0)
    );

    // Column + default `align_items: Stretch` makes children fill
    // the cross-axis (width = 300).
    frame_eq(a.ns_view(), 0.0, 0.0, 300.0, 80.0);
    frame_eq(b.ns_view(), 0.0, 80.0, 300.0, 120.0);
}

// ---------------------------------------------------------------------
// Padding shrinks children's frames inward
// ---------------------------------------------------------------------

fn padding_inset_applies_to_children() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(), layout::FlexDirection::Column
    );
    layout::set_padding(root.as_node(), 16.0);
    let _tree = fresh_tree(&root);

    let child = Element::create("view");
    layout::set_height(child.as_node(), 50.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(200.0, 200.0)
    );

    // Padding 16 on all sides → child positioned at (16,16),
    // width = 200 - 32 = 168.
    frame_eq(child.ns_view(), 16.0, 16.0, 168.0, 50.0);
}

// ---------------------------------------------------------------------
// Gap separates children
// ---------------------------------------------------------------------

fn gap_separates_children() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(), layout::FlexDirection::Column
    );
    layout::set_gap(root.as_node(), 12.0);
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_height(a.as_node(), 30.0);
    layout::set_height(b.as_node(), 40.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(200.0, 200.0)
    );

    frame_eq(a.ns_view(), 0.0, 0.0, 200.0, 30.0);
    // b starts after a's height (30) + gap (12) = y=42.
    frame_eq(b.ns_view(), 0.0, 42.0, 200.0, 40.0);
}

// ---------------------------------------------------------------------
// flex_grow distributes leftover space
// ---------------------------------------------------------------------

fn flex_grow_distributes_leftover() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    // Both grow=1, neither has a width — they should each get half
    // of the available 400.
    layout::set_flex_grow(a.as_node(), 1.0);
    layout::set_flex_grow(b.as_node(), 1.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(400.0, 100.0)
    );

    frame_eq(a.ns_view(), 0.0, 0.0, 200.0, 100.0);
    frame_eq(b.ns_view(), 200.0, 0.0, 200.0, 100.0);
}

fn flex_grow_unequal_distributes_proportionally() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_flex_grow(a.as_node(), 1.0);
    layout::set_flex_grow(b.as_node(), 3.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(
        root.as_node(), NSSize::new(400.0, 100.0)
    );

    // a: 400 * 1/4 = 100; b: 400 * 3/4 = 300.
    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 100.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 300.0, 100.0);
}

// ---------------------------------------------------------------------
// Nested containers
// ---------------------------------------------------------------------

fn nested_containers_inner_fits_within_outer() {
    let _mtm = common::test_mtm();
    let outer = Element::create("view");
    layout::set_flex_direction(
        outer.as_node(), layout::FlexDirection::Column
    );
    layout::set_padding(outer.as_node(), 10.0);
    let _tree = fresh_tree(&outer);

    let inner = Element::create("view");
    layout::set_flex_direction(
        inner.as_node(), layout::FlexDirection::Row
    );
    layout::set_padding(inner.as_node(), 4.0);
    layout::set_height(inner.as_node(), 80.0);
    outer.insert_node(inner.as_node(), None);

    let leaf_a = Element::create("view");
    let leaf_b = Element::create("view");
    layout::set_width(leaf_a.as_node(), 30.0);
    layout::set_width(leaf_b.as_node(), 30.0);
    inner.insert_node(leaf_a.as_node(), None);
    inner.insert_node(leaf_b.as_node(), None);

    layout::compute_layout(
        outer.as_node(), NSSize::new(200.0, 300.0)
    );

    // Outer: full size
    frame_eq(outer.ns_view(), 0.0, 0.0, 200.0, 300.0);
    // Inner: 10px padding on all sides of outer → (10, 10),
    // width = 200 - 20 = 180, height = 80 (explicit).
    frame_eq(inner.ns_view(), 10.0, 10.0, 180.0, 80.0);
    // Leafs: inner padding 4 + their own widths (30 each), in
    // outer's frame coordinates is what NSView::frame reports.
    // Each leaf's frame is local to its parent (inner). We
    // configured leaves with widths 30; inner padded 4 leaves
    // (4, 4)→(34, 4) in inner-local coordinates.
    let inner_subs: Vec<_> =
        inner.ns_view().subviews().iter().collect();
    assert_eq!(inner_subs.len(), 2);
    frame_eq(&*inner_subs[0], 4.0, 4.0, 30.0, 72.0);
    frame_eq(&*inner_subs[1], 34.0, 4.0, 30.0, 72.0);
}

// ---------------------------------------------------------------------
// Edge cases — zero children, zero size
// ---------------------------------------------------------------------

fn zero_children_no_panic() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    let _tree = fresh_tree(&root);
    layout::compute_layout(
        root.as_node(), NSSize::new(100.0, 100.0)
    );
    frame_eq(root.ns_view(), 0.0, 0.0, 100.0, 100.0);
}

fn zero_size_available_no_panic() {
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(), layout::FlexDirection::Row
    );
    let _tree = fresh_tree(&root);
    let child = Element::create("view");
    layout::set_width(child.as_node(), 50.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(root.as_node(), NSSize::new(0.0, 0.0));
    // No assertion on child frame — Taffy may produce 0×0 or its
    // intrinsic. Just verify we didn't panic.
}

fn main() {
    common::run_tests(&[
        ("root_fills_available_space", root_fills_available_space),
        ("row_two_children_side_by_side", row_two_children_side_by_side),
        ("column_two_children_stacked", column_two_children_stacked),
        (
            "padding_inset_applies_to_children",
            padding_inset_applies_to_children,
        ),
        ("gap_separates_children", gap_separates_children),
        ("flex_grow_distributes_leftover", flex_grow_distributes_leftover),
        (
            "flex_grow_unequal_distributes_proportionally",
            flex_grow_unequal_distributes_proportionally,
        ),
        (
            "nested_containers_inner_fits_within_outer",
            nested_containers_inner_fits_within_outer,
        ),
        ("zero_children_no_panic", zero_children_no_panic),
        ("zero_size_available_no_panic", zero_size_available_no_panic),
    ]);
}
