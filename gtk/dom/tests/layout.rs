//! Layout tests — build small trees, set Taffy styles via the
//! gtk_dom helpers, run `compute_layout`, assert frames.
//!
//! Mirrors `cocoa_dom/tests/layout.rs`. Layout is computed against
//! Taffy directly via `layout::compute_layout(root, size)` — the
//! GTK measure/allocate cycle isn't exercised here (that needs a
//! display); we just verify the Taffy bridge sees the right tree.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{layout, Element};

/// Build a fresh layout tree, register `root` as its root, return
/// the tree handle.
fn fresh_tree(root: &Element) -> layout::TreeRef {
    let tree = layout::new_tree();
    layout::register_in_tree(root.as_node(), &tree);
    tree
}

/// Read the Taffy-computed layout for `el` and assert position +
/// size against the expected values.
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
        .mounted_handle()
        .expect("element not registered in tree");
    let layout = tree
        .layout(lh.node_id)
        .expect("no layout computed");
    let tol = 0.5;
    assert!(
        (layout.location.x - x).abs() < tol
            && (layout.location.y - y).abs() < tol
            && (layout.size.width - w).abs() < tol
            && (layout.size.height - h).abs() < tol,
        "frame mismatch: got ({}, {}, {}×{}); expected ({}, {}, {}×{})",
        layout.location.x, layout.location.y,
        layout.size.width, layout.size.height,
        x, y, w, h
    );
}

// ---------------------------------------------------------------------
// Single root sized to fill
// ---------------------------------------------------------------------

fn root_fills_available_space() {
    let root = Element::create("view");
    let tree = fresh_tree(&root);
    layout::compute_layout(root.as_node(), (400.0, 300.0));
    frame_eq(&tree, &root, 0.0, 0.0, 400.0, 300.0);
}

// ---------------------------------------------------------------------
// Row direction — children placed side by side
// ---------------------------------------------------------------------

fn row_two_children_side_by_side() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_width(a.as_node(), 100.0);
    layout::set_height(a.as_node(), 50.0);
    layout::set_width(b.as_node(), 200.0);
    layout::set_height(b.as_node(), 60.0);

    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (500.0, 400.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(&tree, &b, 100.0, 0.0, 200.0, 60.0);
}

// ---------------------------------------------------------------------
// Column direction — children stacked
// ---------------------------------------------------------------------

fn column_two_children_stacked() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_height(a.as_node(), 80.0);
    layout::set_height(b.as_node(), 120.0);

    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (300.0, 500.0));

    // Column + default `align_items: Stretch` makes children fill
    // the cross-axis (width = 300).
    frame_eq(&tree, &a, 0.0, 0.0, 300.0, 80.0);
    frame_eq(&tree, &b, 0.0, 80.0, 300.0, 120.0);
}

// ---------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------

fn padding_inset_applies_to_children() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    layout::set_padding(root.as_node(), 16.0);
    let tree = fresh_tree(&root);

    let child = Element::create("view");
    layout::set_height(child.as_node(), 50.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(root.as_node(), (200.0, 200.0));

    frame_eq(&tree, &child, 16.0, 16.0, 168.0, 50.0);
}

// ---------------------------------------------------------------------
// Gap
// ---------------------------------------------------------------------

fn gap_separates_children() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    layout::set_gap(root.as_node(), 12.0);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_height(a.as_node(), 30.0);
    layout::set_height(b.as_node(), 40.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (200.0, 200.0));

    frame_eq(&tree, &a, 0.0, 0.0, 200.0, 30.0);
    frame_eq(&tree, &b, 0.0, 42.0, 200.0, 40.0);
}

// ---------------------------------------------------------------------
// flex_grow
// ---------------------------------------------------------------------

fn flex_grow_distributes_leftover() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_flex_grow(a.as_node(), 1.0);
    layout::set_flex_grow(b.as_node(), 1.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (400.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 200.0, 100.0);
    frame_eq(&tree, &b, 200.0, 0.0, 200.0, 100.0);
}

fn flex_grow_unequal_distributes_proportionally() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    layout::set_flex_grow(a.as_node(), 1.0);
    layout::set_flex_grow(b.as_node(), 3.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), (400.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 100.0, 100.0);
    frame_eq(&tree, &b, 100.0, 0.0, 300.0, 100.0);
}

fn justify_content_space_between() {
    let root = Element::create("stack");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    layout::set_justify_content(
        root.as_node(),
        layout::JustifyContent::SpaceBetween,
    );
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    for el in [&a, &b, &c] {
        layout::set_width(el.as_node(), 60.0);
        layout::set_height(el.as_node(), 40.0);
        root.insert_node(el.as_node(), None);
    }

    layout::compute_layout(root.as_node(), (600.0, 100.0));

    frame_eq(&tree, &a, 0.0, 0.0, 60.0, 40.0);
    frame_eq(&tree, &b, 270.0, 0.0, 60.0, 40.0);
    frame_eq(&tree, &c, 540.0, 0.0, 60.0, 40.0);
}

fn align_items_center_centres_cross_axis() {
    let root = Element::create("stack");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    layout::set_align_items(root.as_node(), layout::AlignItems::Center);
    let tree = fresh_tree(&root);

    let child = Element::create("view");
    layout::set_width(child.as_node(), 100.0);
    layout::set_height(child.as_node(), 30.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(root.as_node(), (400.0, 200.0));

    frame_eq(&tree, &child, 150.0, 0.0, 100.0, 30.0);
}

// ---------------------------------------------------------------------
// Nested containers
// ---------------------------------------------------------------------

fn nested_containers_inner_fits_within_outer() {
    let outer = Element::create("view");
    layout::set_flex_direction(outer.as_node(), layout::FlexDirection::Column);
    layout::set_padding(outer.as_node(), 10.0);
    let tree = fresh_tree(&outer);

    let inner = Element::create("view");
    layout::set_flex_direction(inner.as_node(), layout::FlexDirection::Row);
    layout::set_padding(inner.as_node(), 4.0);
    layout::set_height(inner.as_node(), 80.0);
    outer.insert_node(inner.as_node(), None);

    let leaf_a = Element::create("view");
    let leaf_b = Element::create("view");
    layout::set_width(leaf_a.as_node(), 30.0);
    layout::set_width(leaf_b.as_node(), 30.0);
    inner.insert_node(leaf_a.as_node(), None);
    inner.insert_node(leaf_b.as_node(), None);

    layout::compute_layout(outer.as_node(), (200.0, 300.0));

    frame_eq(&tree, &outer, 0.0, 0.0, 200.0, 300.0);
    frame_eq(&tree, &inner, 10.0, 10.0, 180.0, 80.0);
    // Leafs: positioned in inner-local Taffy coordinates after
    // inner's padding(4).
    frame_eq(&tree, &leaf_a, 4.0, 4.0, 30.0, 72.0);
    frame_eq(&tree, &leaf_b, 34.0, 4.0, 30.0, 72.0);
}

// ---------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------

fn zero_children_no_panic() {
    let root = Element::create("view");
    let tree = fresh_tree(&root);
    layout::compute_layout(root.as_node(), (100.0, 100.0));
    frame_eq(&tree, &root, 0.0, 0.0, 100.0, 100.0);
}

fn removing_child_collapses_remaining_layout() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    let tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    layout::set_height(a.as_node(), 50.0);
    layout::set_height(b.as_node(), 50.0);
    layout::set_height(c.as_node(), 50.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);
    root.insert_node(c.as_node(), None);

    layout::compute_layout(root.as_node(), (200.0, 200.0));
    frame_eq(&tree, &a, 0.0, 0.0, 200.0, 50.0);
    frame_eq(&tree, &b, 0.0, 50.0, 200.0, 50.0);
    frame_eq(&tree, &c, 0.0, 100.0, 200.0, 50.0);

    root.remove_child(b.as_node());
    layout::compute_layout(root.as_node(), (200.0, 200.0));
    frame_eq(&tree, &a, 0.0, 0.0, 200.0, 50.0);
    frame_eq(&tree, &c, 0.0, 50.0, 200.0, 50.0);
}

fn nested_vstack_collapses_after_removal() {
    let outer = Element::create("view");
    layout::set_flex_direction(outer.as_node(), layout::FlexDirection::Column);
    let tree = fresh_tree(&outer);

    let inner = Element::create("view");
    layout::set_flex_direction(inner.as_node(), layout::FlexDirection::Column);
    let footer = Element::create("view");
    layout::set_height(footer.as_node(), 30.0);

    outer.insert_node(inner.as_node(), None);
    outer.insert_node(footer.as_node(), None);

    let row_a = Element::create("view");
    let row_b = Element::create("view");
    let row_c = Element::create("view");
    layout::set_height(row_a.as_node(), 40.0);
    layout::set_height(row_b.as_node(), 40.0);
    layout::set_height(row_c.as_node(), 40.0);

    inner.insert_node(row_a.as_node(), None);
    inner.insert_node(row_b.as_node(), None);
    inner.insert_node(row_c.as_node(), None);

    layout::compute_layout(outer.as_node(), (300.0, 400.0));
    frame_eq(&tree, &footer, 0.0, 120.0, 300.0, 30.0);

    inner.remove_child(row_b.as_node());
    layout::compute_layout(outer.as_node(), (300.0, 400.0));
    frame_eq(&tree, &footer, 0.0, 80.0, 300.0, 30.0);
}

/// REGRESSION: when a label's text changes from "0" to "-1" the
/// label's intrinsic width should grow, and the next compute_layout
/// pass should reflect the new width — not the cached one. If the
/// dirty bit isn't propagating, Taffy returns the old layout and the
/// label is allocated too narrow, forcing GTK Label to wrap "-1" to
/// two lines.
fn label_text_change_reflowed_on_relayout() {
    use gtk4::prelude::*;
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let _tree = fresh_tree(&root);

    let label = Element::create("label");
    label.set_attribute("value", "0");
    root.insert_node(label.as_node(), None);

    layout::compute_layout(root.as_node(), (300.0, 50.0));
    let lh = label
        .as_node()
        .mounted_handle()
        .expect("label registered");
    let w_zero = _tree.layout(lh.node_id).expect("layout computed").size.width;
    let raw_zero = label
        .widget()
        .downcast_ref::<gtk4::Label>()
        .map(|l| l.measure(gtk4::Orientation::Horizontal, -1).1)
        .unwrap_or(-1);

    label.set_attribute("value", "-1");
    let raw_minus = label
        .widget()
        .downcast_ref::<gtk4::Label>()
        .map(|l| l.measure(gtk4::Orientation::Horizontal, -1).1)
        .unwrap_or(-1);
    layout::compute_layout(root.as_node(), (300.0, 50.0));
    let w_minus_one = _tree.layout(lh.node_id).expect("layout computed").size.width;

    assert!(
        w_minus_one >= raw_minus as f32,
        "Taffy width {w_minus_one} should at least match GTK natural \
         width {raw_minus} after text change. (raw \"0\" -> \"-1\": \
         {raw_zero} -> {raw_minus}; Taffy: {w_zero} -> {w_minus_one})"
    );
}

fn zero_size_available_no_panic() {
    let root = Element::create("view");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    let _tree = fresh_tree(&root);
    let child = Element::create("view");
    layout::set_width(child.as_node(), 50.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(root.as_node(), (0.0, 0.0));
    // No assertion on child frame — Taffy may produce 0×0. Just
    // verify we didn't panic.
}

fn main() {
    common::run_tests(&[
        ("root_fills_available_space", root_fills_available_space),
        ("row_two_children_side_by_side", row_two_children_side_by_side),
        ("column_two_children_stacked", column_two_children_stacked),
        ("padding_inset_applies_to_children", padding_inset_applies_to_children),
        ("gap_separates_children", gap_separates_children),
        ("flex_grow_distributes_leftover", flex_grow_distributes_leftover),
        (
            "flex_grow_unequal_distributes_proportionally",
            flex_grow_unequal_distributes_proportionally,
        ),
        ("justify_content_space_between", justify_content_space_between),
        ("align_items_center_centres_cross_axis", align_items_center_centres_cross_axis),
        (
            "nested_containers_inner_fits_within_outer",
            nested_containers_inner_fits_within_outer,
        ),
        ("zero_children_no_panic", zero_children_no_panic),
        (
            "label_text_change_reflowed_on_relayout",
            label_text_change_reflowed_on_relayout,
        ),
        (
            "removing_child_collapses_remaining_layout",
            removing_child_collapses_remaining_layout,
        ),
        (
            "nested_vstack_collapses_after_removal",
            nested_vstack_collapses_after_removal,
        ),
        ("zero_size_available_no_panic", zero_size_available_no_panic),
        ]);
}
