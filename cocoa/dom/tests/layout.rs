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

fn justify_content_space_between() {
    let _mtm = common::test_mtm();
    let root = Element::create("stack");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Row);
    layout::set_justify_content(
        root.as_node(),
        layout::JustifyContent::SpaceBetween,
    );
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    for el in [&a, &b, &c] {
        layout::set_width(el.as_node(), 60.0);
        layout::set_height(el.as_node(), 40.0);
        root.insert_node(el.as_node(), None);
    }

    layout::compute_layout(root.as_node(), NSSize::new(600.0, 100.0));

    // 600 - 3*60 = 420 leftover, distributed as gaps between siblings.
    frame_eq(a.ns_view(), 0.0, 0.0, 60.0, 40.0);
    frame_eq(b.ns_view(), 270.0, 0.0, 60.0, 40.0);
    frame_eq(c.ns_view(), 540.0, 0.0, 60.0, 40.0);
}

fn align_items_center_centres_cross_axis() {
    let _mtm = common::test_mtm();
    let root = Element::create("stack");
    layout::set_flex_direction(root.as_node(), layout::FlexDirection::Column);
    layout::set_align_items(root.as_node(), layout::AlignItems::Center);
    let _tree = fresh_tree(&root);

    let child = Element::create("view");
    layout::set_width(child.as_node(), 100.0);
    layout::set_height(child.as_node(), 30.0);
    root.insert_node(child.as_node(), None);

    layout::compute_layout(root.as_node(), NSSize::new(400.0, 200.0));

    // Centred in a 400-wide container: x = (400 - 100) / 2 = 150.
    frame_eq(child.ns_view(), 150.0, 0.0, 100.0, 30.0);
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

fn removing_child_collapses_remaining_layout() {
    // Regression: when a row was removed via `remove_child` /
    // teardown, Taffy's cached layout for the parent stayed valid
    // and the next compute_layout would keep allocating space for
    // the now-removed children. Fixed in cocoa_dom::layout by
    // explicitly mark_dirty-ing the parent on remove_child /
    // drop_node.
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(),
        layout::FlexDirection::Column,
    );
    let _tree = fresh_tree(&root);

    let a = Element::create("view");
    let b = Element::create("view");
    let c = Element::create("view");
    layout::set_height(a.as_node(), 50.0);
    layout::set_height(b.as_node(), 50.0);
    layout::set_height(c.as_node(), 50.0);
    root.insert_node(a.as_node(), None);
    root.insert_node(b.as_node(), None);
    root.insert_node(c.as_node(), None);

    layout::compute_layout(
        root.as_node(),
        NSSize::new(200.0, 200.0),
    );
    frame_eq(a.ns_view(), 0.0, 0.0, 200.0, 50.0);
    frame_eq(b.ns_view(), 0.0, 50.0, 200.0, 50.0);
    frame_eq(c.ns_view(), 0.0, 100.0, 200.0, 50.0);

    // Remove the middle child — `c` should rise to where `b` was.
    root.remove_child(b.as_node());
    layout::compute_layout(
        root.as_node(),
        NSSize::new(200.0, 200.0),
    );
    frame_eq(a.ns_view(), 0.0, 0.0, 200.0, 50.0);
    frame_eq(c.ns_view(), 0.0, 50.0, 200.0, 50.0);
}

fn scroll_view_bounds_parent_to_viewport() {
    // Regression for the bug where scroll_view used to inherit its
    // content's natural height (e.g. 1000) into the layout, forcing
    // its parent to grow past the window. With flex_basis=0 +
    // min_size=0 + overflow:Hidden, scroll_view's allocated frame
    // matches the viewport (its parent's allocated space), and the
    // second-pass `compute_layout` sets the documentView's frame to
    // the natural content size for NSScrollView to scroll.
    let _mtm = common::test_mtm();
    let root = Element::create("view");
    layout::set_flex_direction(
        root.as_node(),
        layout::FlexDirection::Column,
    );
    let _tree = fresh_tree(&root);

    let scroll = Element::create("scroll_view");
    layout::set_flex_grow(scroll.as_node(), 1.0);
    root.insert_node(scroll.as_node(), None);

    // Add a single tall child — 30 rows of 16 high each = 480 total
    // (plus default gap of 0). Without the layout fix this would
    // bubble up to the root and overflow; with it, scroll_view's
    // own frame stays at the viewport size (root's allotted space).
    let inner = Element::create("view");
    layout::set_flex_direction(
        inner.as_node(),
        layout::FlexDirection::Column,
    );
    scroll.insert_node(inner.as_node(), None);
    for _ in 0..30 {
        let row = Element::create("view");
        layout::set_height(row.as_node(), 16.0);
        inner.insert_node(row.as_node(), None);
    }

    layout::compute_layout(
        root.as_node(),
        NSSize::new(200.0, 200.0),
    );

    // scroll_view's NSView frame = viewport (the 200×200 window
    // minus zero padding). NOT the natural content height (480).
    frame_eq(scroll.ns_view(), 0.0, 0.0, 200.0, 200.0);
    // documentView (an NSScrollView's first subview) is the part
    // that grows to natural content height. The window's frame
    // doesn't grow past 200.
    frame_eq(root.ns_view(), 0.0, 0.0, 200.0, 200.0);
}

fn nested_vstack_collapses_after_removal() {
    // Mimics the todomvc layout: outer vstack contains an inner
    // vstack of "rows" plus a "footer" sibling. Removing rows
    // should make the footer's y-coordinate move up — the bug
    // would leave the footer at its original y (because the inner
    // vstack didn't shrink in the cached layout).
    let _mtm = common::test_mtm();
    let outer = Element::create("view");
    layout::set_flex_direction(
        outer.as_node(),
        layout::FlexDirection::Column,
    );
    let _tree = fresh_tree(&outer);

    let inner = Element::create("view");
    layout::set_flex_direction(
        inner.as_node(),
        layout::FlexDirection::Column,
    );
    let footer = Element::create("view");
    layout::set_height(footer.as_node(), 30.0);

    // Register parent → child top-down: `attach_child` is a no-op
    // if the parent isn't in the tree yet, so register the
    // hierarchy in mount order.
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

    layout::compute_layout(
        outer.as_node(),
        NSSize::new(300.0, 400.0),
    );
    // Inner vstack height = 3 rows × 40 = 120, then footer
    // immediately below at y=120.
    frame_eq(footer.ns_view(), 0.0, 120.0, 300.0, 30.0);

    // Remove the middle row — inner should shrink to 80, footer
    // should slide up to y=80.
    inner.remove_child(row_b.as_node());
    layout::compute_layout(
        outer.as_node(),
        NSSize::new(300.0, 400.0),
    );
    frame_eq(footer.ns_view(), 0.0, 80.0, 300.0, 30.0);
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
        ("justify_content_space_between", justify_content_space_between),
        ("align_items_center_centres_cross_axis", align_items_center_centres_cross_axis),
        (
            "nested_containers_inner_fits_within_outer",
            nested_containers_inner_fits_within_outer,
        ),
        ("zero_children_no_panic", zero_children_no_panic),
        (
            "scroll_view_bounds_parent_to_viewport",
            scroll_view_bounds_parent_to_viewport,
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
