//! Layout tests — build small trees, set Taffy styles via the
//! gtk_dom helpers, run `compute_layout`, assert frames.
//!
//! Mirrors `cocoa_dom/tests/layout.rs`. Layout is computed against
//! Taffy directly via `layout::compute_layout(root, size)` — the
//! GTK measure/allocate cycle isn't exercised here (that needs a
//! display); we just verify the Taffy bridge sees the right tree.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::dom::{layout, GtkElem, layout::GtkBackend};
use leptos_native::renderer::scene::LayoutBackend;

/// Read the Taffy-computed layout for `el` and assert position +
/// size against the expected values.
fn frame_eq(
    el: GtkElem,
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
        l.location.x, l.location.y,
        l.size.width, l.size.height,
        x, y, w, h
    );
}

// ---------------------------------------------------------------------
// Single root sized to fill
// ---------------------------------------------------------------------

fn root_fills_available_space() {
    let root = GtkElem::create_stack();
    layout::compute_layout(root, (400.0, 300.0));
    frame_eq(root, 0.0, 0.0, 400.0, 300.0);
}

// ---------------------------------------------------------------------
// Row direction — children placed side by side
// ---------------------------------------------------------------------

fn row_two_children_side_by_side() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_width(a, 100.0);
    layout::set_height(a, 50.0);
    layout::set_width(b, 200.0);
    layout::set_height(b, 60.0);

    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (500.0, 400.0));

    frame_eq(a, 0.0, 0.0, 100.0, 50.0);
    frame_eq(b, 100.0, 0.0, 200.0, 60.0);
}

// ---------------------------------------------------------------------
// Column direction — children stacked
// ---------------------------------------------------------------------

fn column_two_children_stacked() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Column);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_height(a, 80.0);
    layout::set_height(b, 120.0);

    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (300.0, 500.0));

    // Column + default `align_items: Stretch` makes children fill
    // the cross-axis (width = 300).
    frame_eq(a, 0.0, 0.0, 300.0, 80.0);
    frame_eq(b, 0.0, 80.0, 300.0, 120.0);
}

// ---------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------

fn padding_inset_applies_to_children() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Column);
    layout::set_padding(root, 16.0);

    let child = GtkElem::create_stack();
    layout::set_height(child, 50.0);
    root.insert_node(child, None);

    layout::compute_layout(root, (200.0, 200.0));

    frame_eq(child, 16.0, 16.0, 168.0, 50.0);
}

// ---------------------------------------------------------------------
// Gap
// ---------------------------------------------------------------------

fn gap_separates_children() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Column);
    layout::set_gap(root, 12.0);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_height(a, 30.0);
    layout::set_height(b, 40.0);
    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (200.0, 200.0));

    frame_eq(a, 0.0, 0.0, 200.0, 30.0);
    frame_eq(b, 0.0, 42.0, 200.0, 40.0);
}

// ---------------------------------------------------------------------
// flex_grow
// ---------------------------------------------------------------------

fn flex_grow_distributes_leftover() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_flex_grow(a, 1.0);
    layout::set_flex_grow(b, 1.0);
    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (400.0, 100.0));

    frame_eq(a, 0.0, 0.0, 200.0, 100.0);
    frame_eq(b, 200.0, 0.0, 200.0, 100.0);
}

fn flex_grow_unequal_distributes_proportionally() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    layout::set_flex_grow(a, 1.0);
    layout::set_flex_grow(b, 3.0);
    root.insert_node(a, None);
    root.insert_node(b, None);

    layout::compute_layout(root, (400.0, 100.0));

    frame_eq(a, 0.0, 0.0, 100.0, 100.0);
    frame_eq(b, 100.0, 0.0, 300.0, 100.0);
}

fn justify_content_space_between() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);
    layout::set_justify_content(
        root,
        layout::JustifyContent::SpaceBetween,
    );

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    for el in [a, b, c] {
        layout::set_width(el, 60.0);
        layout::set_height(el, 40.0);
        root.insert_node(el, None);
    }

    layout::compute_layout(root, (600.0, 100.0));

    frame_eq(a, 0.0, 0.0, 60.0, 40.0);
    frame_eq(b, 270.0, 0.0, 60.0, 40.0);
    frame_eq(c, 540.0, 0.0, 60.0, 40.0);
}

fn align_items_center_centres_cross_axis() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Column);
    layout::set_align_items(root, layout::AlignItems::Center);

    let child = GtkElem::create_stack();
    layout::set_width(child, 100.0);
    layout::set_height(child, 30.0);
    root.insert_node(child, None);

    layout::compute_layout(root, (400.0, 200.0));

    frame_eq(child, 150.0, 0.0, 100.0, 30.0);
}

// ---------------------------------------------------------------------
// Nested containers
// ---------------------------------------------------------------------

fn nested_containers_inner_fits_within_outer() {
    let outer = GtkElem::create_stack();
    layout::set_flex_direction(outer, layout::FlexDirection::Column);
    layout::set_padding(outer, 10.0);

    let inner = GtkElem::create_stack();
    layout::set_flex_direction(inner, layout::FlexDirection::Row);
    layout::set_padding(inner, 4.0);
    layout::set_height(inner, 80.0);
    outer.insert_node(inner, None);

    let leaf_a = GtkElem::create_stack();
    let leaf_b = GtkElem::create_stack();
    layout::set_width(leaf_a, 30.0);
    layout::set_width(leaf_b, 30.0);
    inner.insert_node(leaf_a, None);
    inner.insert_node(leaf_b, None);

    layout::compute_layout(outer, (200.0, 300.0));

    frame_eq(outer, 0.0, 0.0, 200.0, 300.0);
    frame_eq(inner, 10.0, 10.0, 180.0, 80.0);
    // Leafs: positioned in inner-local Taffy coordinates after
    // inner's padding(4).
    frame_eq(leaf_a, 4.0, 4.0, 30.0, 72.0);
    frame_eq(leaf_b, 34.0, 4.0, 30.0, 72.0);
}

// ---------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------

fn zero_children_no_panic() {
    let root = GtkElem::create_stack();
    layout::compute_layout(root, (100.0, 100.0));
    frame_eq(root, 0.0, 0.0, 100.0, 100.0);
}

fn removing_child_collapses_remaining_layout() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Column);

    let a = GtkElem::create_stack();
    let b = GtkElem::create_stack();
    let c = GtkElem::create_stack();
    layout::set_height(a, 50.0);
    layout::set_height(b, 50.0);
    layout::set_height(c, 50.0);
    root.insert_node(a, None);
    root.insert_node(b, None);
    root.insert_node(c, None);

    layout::compute_layout(root, (200.0, 200.0));
    frame_eq(a, 0.0, 0.0, 200.0, 50.0);
    frame_eq(b, 0.0, 50.0, 200.0, 50.0);
    frame_eq(c, 0.0, 100.0, 200.0, 50.0);

    root.remove_child(b);
    layout::compute_layout(root, (200.0, 200.0));
    frame_eq(a, 0.0, 0.0, 200.0, 50.0);
    frame_eq(c, 0.0, 50.0, 200.0, 50.0);
}

fn nested_vstack_collapses_after_removal() {
    let outer = GtkElem::create_stack();
    layout::set_flex_direction(outer, layout::FlexDirection::Column);

    let inner = GtkElem::create_stack();
    layout::set_flex_direction(inner, layout::FlexDirection::Column);
    let footer = GtkElem::create_stack();
    layout::set_height(footer, 30.0);

    outer.insert_node(inner, None);
    outer.insert_node(footer, None);

    let row_a = GtkElem::create_stack();
    let row_b = GtkElem::create_stack();
    let row_c = GtkElem::create_stack();
    layout::set_height(row_a, 40.0);
    layout::set_height(row_b, 40.0);
    layout::set_height(row_c, 40.0);

    inner.insert_node(row_a, None);
    inner.insert_node(row_b, None);
    inner.insert_node(row_c, None);

    layout::compute_layout(outer, (300.0, 400.0));
    frame_eq(footer, 0.0, 120.0, 300.0, 30.0);

    inner.remove_child(row_b);
    layout::compute_layout(outer, (300.0, 400.0));
    frame_eq(footer, 0.0, 80.0, 300.0, 30.0);
}

/// REGRESSION: when a label's text changes from "0" to "-1" the
/// label's intrinsic width should grow, and the next compute_layout
/// pass should reflect the new width — not the cached one. If the
/// dirty bit isn't propagating, Taffy returns the old layout and the
/// label is allocated too narrow, forcing GTK Label to wrap "-1" to
/// two lines.
fn label_text_change_reflowed_on_relayout() {
    use gtk4::prelude::*;
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);

    let label = GtkElem::create_label().0;
    label.set_value("0");
    root.insert_node(label, None);

    layout::compute_layout(root, (300.0, 50.0));
    let label_id = label.id();
    let w_zero = GtkBackend::layout(label_id).expect("layout computed").size.width;
    let raw_zero = label
        .widget()
        .downcast_ref::<gtk4::Label>()
        .map(|l| l.measure(gtk4::Orientation::Horizontal, -1).1)
        .unwrap_or(-1);

    label.set_value("-1");
    let raw_minus = label
        .widget()
        .downcast_ref::<gtk4::Label>()
        .map(|l| l.measure(gtk4::Orientation::Horizontal, -1).1)
        .unwrap_or(-1);
    layout::compute_layout(root, (300.0, 50.0));
    let w_minus_one = GtkBackend::layout(label_id).expect("layout computed").size.width;

    assert!(
        w_minus_one >= raw_minus as f32,
        "Taffy width {w_minus_one} should at least match GTK natural \
         width {raw_minus} after text change. (raw \"0\" -> \"-1\": \
         {raw_zero} -> {raw_minus}; Taffy: {w_zero} -> {w_minus_one})"
    );
}

fn zero_size_available_no_panic() {
    let root = GtkElem::create_stack();
    layout::set_flex_direction(root, layout::FlexDirection::Row);
    let child = GtkElem::create_stack();
    layout::set_width(child, 50.0);
    root.insert_node(child, None);

    layout::compute_layout(root, (0.0, 0.0));
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
