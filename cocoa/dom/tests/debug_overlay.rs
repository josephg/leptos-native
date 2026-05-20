//! Tests for the `debug-overlay` feature.
//!
//! REGRESSION: installing the overlay added a non-Taffy subview at
//! the top of `content_root.subviews()`. `apply_layout` walked
//! subviews by index, which made every Taffy child consume the wrong
//! NSView — content stacked up in the top-left corner.
//!
//! These tests pin the contract: with the overlay installed, layout
//! still places real children at their Taffy-computed frames.

#![cfg(all(target_os = "macos", feature = "debug-overlay"))]

mod common;

use cocoa_dom::{debug_overlay, flipped_view::FlippedView, layout, Element};
use objc2_app_kit::NSView;
use objc2_foundation::NSSize;

fn frame_eq(view: &NSView, x: f64, y: f64, w: f64, h: f64) {
    let f = view.frame();
    let tol = 0.5;
    assert!(
        (f.origin.x - x).abs() < tol
            && (f.origin.y - y).abs() < tol
            && (f.size.width - w).abs() < tol
            && (f.size.height - h).abs() < tol,
        "frame mismatch: got ({}, {}, {}×{}); expected ({}, {}, {}×{})",
        f.origin.x, f.origin.y, f.size.width, f.size.height, x, y, w, h
    );
}

/// With the overlay installed, two row children should still land at
/// their flexbox-computed positions — not be shoved into the corner
/// because the overlay shifted subview indices.
fn overlay_does_not_shift_children() {
    let mtm = common::test_mtm();
    let root = Element::create_container_with(mtm);
    layout::set_flex_direction(
        root.as_node(),
        layout::FlexDirection::Row,
    );
    layout::set_as_root(root.as_node());

    // Install the overlay BEFORE adding children — same order
    // window.rs does.
    let __nv = root.ns_view();
    let any: &objc2::runtime::AnyObject = __nv.as_ref();
    let flipped = any
        .downcast_ref::<FlippedView>()
        .expect("\"view\" tag should produce a FlippedView");
    debug_overlay::install(flipped, &tree, mtm);

    let a = Element::create_container_with(mtm);
    let b = Element::create_container_with(mtm);
    layout::set_width(a.as_node(), 100.0);
    layout::set_height(a.as_node(), 50.0);
    layout::set_width(b.as_node(), 200.0);
    layout::set_height(b.as_node(), 60.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);

    layout::compute_layout(root.as_node(), NSSize::new(500.0, 400.0));

    // The bug: without the apply_layout overlay-skip, both children
    // would inherit the overlay's full-bounds frame and stack at the
    // origin.
    frame_eq(a.ns_view(), 0.0, 0.0, 100.0, 50.0);
    frame_eq(b.ns_view(), 100.0, 0.0, 200.0, 60.0);
}

/// Same as above, but using insert-before-marker (the path that
/// computes a child_index from the subview list). The overlay must
/// also be skipped from that index calculation, otherwise inserts
/// land at wrong Taffy indices.
fn overlay_does_not_shift_marker_inserts() {
    let mtm = common::test_mtm();
    let root = Element::create_container_with(mtm);
    layout::set_flex_direction(
        root.as_node(),
        layout::FlexDirection::Row,
    );
    layout::set_as_root(root.as_node());

    let __nv = root.ns_view();
    let any: &objc2::runtime::AnyObject = __nv.as_ref();
    let flipped = any.downcast_ref::<FlippedView>().unwrap();
    debug_overlay::install(flipped, &tree, mtm);

    let a = Element::create_container_with(mtm);
    let b = Element::create_container_with(mtm);
    let c = Element::create_container_with(mtm);
    for (el, w) in [(&a, 50.0), (&b, 60.0), (&c, 70.0)] {
        layout::set_width(el.as_node(), w);
        layout::set_height(el.as_node(), 40.0);
    }

    // Insert a, then c, then b BEFORE c.
    root.insert_node(a.as_node(), None);
    root.insert_node(c.as_node(), None);
    root.insert_node(b.as_node(), Some(c.as_node()));

    layout::compute_layout(root.as_node(), NSSize::new(500.0, 200.0));

    // Expected ordering in the row: a, b, c.
    frame_eq(a.ns_view(), 0.0, 0.0, 50.0, 40.0);
    frame_eq(b.ns_view(), 50.0, 0.0, 60.0, 40.0);
    frame_eq(c.ns_view(), 110.0, 0.0, 70.0, 40.0);
}

fn main() {
    common::run_tests(&[
        ("overlay_does_not_shift_children", overlay_does_not_shift_children),
        (
            "overlay_does_not_shift_marker_inserts",
            overlay_does_not_shift_marker_inserts,
        ),
    ]);
}
