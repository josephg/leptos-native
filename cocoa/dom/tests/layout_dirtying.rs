//! mark_dirty discipline tests.
//!
//! Per CLAUDE.md (and `feedback_taffy_mark_dirty.md`), every
//! cocoa_dom layout mutation must explicitly mark_dirty its parent
//! before scheduling relayout. These tests pin that contract by
//! reading Taffy's dirty bit directly after each kind of mutation.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{
    layout::{compute_layout, set_as_root, TreeRef},
    Element,
};
use objc2_foundation::NSSize;

fn fresh_tree() -> TreeRef {
    cocoa_dom::layout::new_tree()
}

fn dirty_for(tree: &TreeRef, el: &Element) -> bool {
    let lh = el
        .as_node()
        .mounted_handle()
        .expect("element has no LayoutHandle — wasn't registered");
    tree.dirty(lh.node_id)
}

/// After `compute_layout`, the root's dirty bit is cleared.
/// Subsequent mutations need to set it back.
fn baseline_compute_clears_dirty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);

    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root), "root still dirty after compute");
}

/// Inserting a child marks the parent dirty.
fn attach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    let child = Element::create_button(&tree).0;
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after attach_child"
    );
}

/// Detaching a child marks the parent dirty (so the parent re-runs
/// flexbox without the removed child).
fn detach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    let child = Element::create_button(&tree).0;
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    cocoa_dom::layout::detach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after detach_child"
    );
}

/// Setting an attribute that affects size/text marks the node
/// dirty so its measure callback re-runs.
fn set_text_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    let child = Element::create_label(&tree).0;
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &child));

    child.set_attribute("value", "now I have content");

    assert!(
        dirty_for(&tree, &child),
        "label not marked dirty after text change — measure cache will \
         be stale"
    );
}

/// `set_style` (e.g. width / padding) marks the node dirty.
fn set_style_width_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    compute_layout(root.as_node(), NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    cocoa_dom::layout::set_width(root.as_node(), 150.0);

    assert!(
        dirty_for(&tree, &root),
        "node not marked dirty after set_width"
    );
}

// ---------------------------------------------------------------------
// Idempotency: re-attaching an already-attached child must not produce
// a duplicate parent->child edge in the Taffy tree.
//
// REGRESSION: keyed `<For>` reorders re-call `attach_child` /
// `insert_child_at` with the same node. Without dedup, every move
// would duplicate the row in Taffy, blowing out the parent's flex
// content size and shoving siblings off-screen.
// ---------------------------------------------------------------------

fn child_count(tree: &TreeRef, parent: &Element) -> usize {
    let lh = parent
        .as_node()
        .mounted_handle()
        .expect("element has no LayoutHandle");
    tree.children(lh.node_id).len()
}

fn attach_child_is_idempotent() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    let child = Element::create_button(&tree).0;

    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(child_count(&tree, &root), 1);
    cocoa_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(
        child_count(&tree, &root),
        1,
        "attach_child duplicated the parent->child edge"
    );
}

fn insert_child_at_is_idempotent() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);
    let a = Element::create_button(&tree).0;
    let b = Element::create_button(&tree).0;

    cocoa_dom::layout::insert_child_at(root.as_node(), a.as_node(), 0);
    cocoa_dom::layout::insert_child_at(root.as_node(), b.as_node(), 1);
    assert_eq!(child_count(&tree, &root), 2);

    // Re-insert `a` at position 1 — should reorder, not duplicate.
    cocoa_dom::layout::insert_child_at(root.as_node(), a.as_node(), 1);
    assert_eq!(
        child_count(&tree, &root),
        2,
        "insert_child_at duplicated the parent->child edge"
    );

    // Order should be [b, a] now.
    let lh = root.as_node().mounted_handle().unwrap();
    let a_id = a.as_node().tree_id().unwrap().1;
    let b_id = b.as_node().tree_id().unwrap().1;
    assert_eq!(
        *tree.children(lh.node_id),
        [b_id, a_id],
        "child order wrong after reorder"
    );
}

/// Replays the operations a keyed-`<For>` move performs against the
/// Taffy tree (the same calls a `Mountable::mount` cascade emits when
/// re-mounting under the same parent at a new position): re-attach
/// every child, plus an explicit `insert_child_at` for the relocated
/// one. After the dust settles the tree must still have exactly the
/// original three edges — not three edges per row.
fn reorder_cascade_does_not_duplicate_edges() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let mtm = common::test_mtm();
    let tree = fresh_tree();
    let root = Element::create_container_with(&tree, mtm);
    set_as_root(root.as_node(), &tree);

    let a = Element::create_button(&tree).0;
    let b = Element::create_button(&tree).0;
    let c = Element::create_button(&tree).0;
    cocoa_dom::layout::attach_child(root.as_node(), a.as_node());
    cocoa_dom::layout::attach_child(root.as_node(), b.as_node());
    cocoa_dom::layout::attach_child(root.as_node(), c.as_node());
    assert_eq!(child_count(&tree, &root), 3);

    // Move `a` to position 2, then a remount cascade re-attaches the
    // others to their existing parent.
    cocoa_dom::layout::insert_child_at(root.as_node(), a.as_node(), 2);
    cocoa_dom::layout::attach_child(root.as_node(), b.as_node());
    cocoa_dom::layout::attach_child(root.as_node(), c.as_node());

    assert_eq!(
        child_count(&tree, &root),
        3,
        "reorder duplicated parent->child edges in Taffy"
    );
}

fn main() {
    common::run_tests(&[
        ("baseline_compute_clears_dirty", baseline_compute_clears_dirty),
        ("attach_child_marks_parent_dirty", attach_child_marks_parent_dirty),
        ("detach_child_marks_parent_dirty", detach_child_marks_parent_dirty),
        ("set_text_marks_node_dirty", set_text_marks_node_dirty),
        ("set_style_width_marks_node_dirty", set_style_width_marks_node_dirty),
        ("attach_child_is_idempotent", attach_child_is_idempotent),
        ("insert_child_at_is_idempotent", insert_child_at_is_idempotent),
        (
            "reorder_cascade_does_not_duplicate_edges",
            reorder_cascade_does_not_duplicate_edges,
        ),
    ]);
}

