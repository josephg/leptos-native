//! mark_dirty discipline tests.
//!
//! Per CLAUDE.md (and `feedback_taffy_mark_dirty.md`), every
//! cocoa_dom layout mutation must explicitly mark_dirty its parent
//! before scheduling relayout. These tests pin that contract by
//! reading Taffy's dirty bit directly after each kind of mutation.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::{layout, layout::compute_layout, CocoaElem};
use objc2_foundation::NSSize;

fn dirty_for(el: &CocoaElem) -> bool {
    layout::dirty(el.id())
}

/// After `compute_layout`, the root's dirty bit is cleared.
/// Subsequent mutations need to set it back.
fn baseline_compute_clears_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);

    compute_layout(root, NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&root), "root still dirty after compute");
}

/// Inserting a child marks the parent dirty.
fn attach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    compute_layout(root, NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&root));

    let child = CocoaElem::create_button().0;
    layout::attach_child(root, child);

    assert!(
        dirty_for(&root),
        "parent not marked dirty after attach_child"
    );
}

/// Detaching a child marks the parent dirty (so the parent re-runs
/// flexbox without the removed child).
fn detach_child_marks_parent_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    let child = CocoaElem::create_button().0;
    layout::attach_child(root, child);
    compute_layout(root, NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&root));

    layout::detach_child(root, child);

    assert!(
        dirty_for(&root),
        "parent not marked dirty after detach_child"
    );
}

/// Setting an attribute that affects size/text marks the node
/// dirty so its measure callback re-runs.
fn set_text_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    let child = CocoaElem::create_label().0;
    layout::attach_child(root, child);
    compute_layout(root, NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&child));

    child.set_value("now I have content");

    assert!(
        dirty_for(&child),
        "label not marked dirty after text change — measure cache will \
         be stale"
    );
}

/// `set_style` (e.g. width / padding) marks the node dirty.
fn set_style_width_marks_node_dirty() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    compute_layout(root, NSSize::new(200.0, 200.0));
    assert!(!dirty_for(&root));

    layout::set_width(root, 150.0);

    assert!(
        dirty_for(&root),
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

fn child_count(parent: &CocoaElem) -> usize {
    layout::children(parent.id()).len()
}

fn attach_child_is_idempotent() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    let child = CocoaElem::create_button().0;

    layout::attach_child(root, child);
    assert_eq!(child_count(&root), 1);
    layout::attach_child(root, child);
    assert_eq!(
        child_count(&root),
        1,
        "attach_child duplicated the parent->child edge"
    );
}

fn insert_child_at_is_idempotent() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);
    let a = CocoaElem::create_button().0;
    let b = CocoaElem::create_button().0;

    layout::insert_child_at(root, a, 0);
    layout::insert_child_at(root, b, 1);
    assert_eq!(child_count(&root), 2);

    // Re-insert `a` at position 1 — should reorder, not duplicate.
    layout::insert_child_at(root, a, 1);
    assert_eq!(
        child_count(&root),
        2,
        "insert_child_at duplicated the parent->child edge"
    );

    // Order should be [b, a] now.
    let a_id = a.id();
    let b_id = b.id();
    assert_eq!(
        layout::children(root.id()),
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
    let mtm = common::test_mtm();
    let root = CocoaElem::create_container_with(mtm);

    let a = CocoaElem::create_button().0;
    let b = CocoaElem::create_button().0;
    let c = CocoaElem::create_button().0;
    layout::attach_child(root, a);
    layout::attach_child(root, b);
    layout::attach_child(root, c);
    assert_eq!(child_count(&root), 3);

    // Move `a` to position 2, then a remount cascade re-attaches the
    // others to their existing parent.
    layout::insert_child_at(root, a, 2);
    layout::attach_child(root, b);
    layout::attach_child(root, c);

    assert_eq!(
        child_count(&root),
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

