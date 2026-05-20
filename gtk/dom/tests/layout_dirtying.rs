//! mark_dirty discipline tests — every mutation must invalidate the
//! Taffy cache so re-layout sees fresh measurements.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{layout, layout::compute_layout, Node};

fn dirty_for(el: &Node) -> bool {
    layout::dirty(el.as_node().id())
}

/// After `compute_layout`, the root's dirty bit is cleared.
fn baseline_compute_clears_dirty() {
    let root = Node::create_vstack();

    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root), "root still dirty after compute");
}

fn attach_child_marks_parent_dirty() {
    let root = Node::create_vstack();
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    let child = Node::create_button().0;
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&root),
        "parent not marked dirty after attach_child"
    );
}

fn detach_child_marks_parent_dirty() {
    let root = Node::create_vstack();
    let child = Node::create_button().0;
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    gtk_dom::layout::detach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&root),
        "parent not marked dirty after detach_child"
    );
}

fn set_text_marks_node_dirty() {
    let root = Node::create_vstack();
    let child = Node::create_label().0;
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&child));

    child.set_value("now I have content");

    assert!(
        dirty_for(&child),
        "label not marked dirty after text change"
    );
}

fn set_style_width_marks_node_dirty() {
    let root = Node::create_vstack();
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    gtk_dom::layout::set_width(root.as_node(), 150.0);

    assert!(
        dirty_for(&root),
        "node not marked dirty after set_width"
    );
}

// ---------------------------------------------------------------------
// Idempotency: re-attaching an already-attached child must not produce
// a duplicate parent->child edge in the Taffy tree.
// ---------------------------------------------------------------------

fn child_count(parent: &Node) -> usize {
    layout::children(parent.as_node().id()).len()
}

fn attach_child_is_idempotent() {
    let root = Node::create_vstack();
    let child = Node::create_button().0;

    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(child_count(&root), 1);
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(
        child_count(&root),
        1,
        "attach_child duplicated the parent->child edge"
    );
}

fn insert_child_at_is_idempotent() {
    let root = Node::create_vstack();
    let a = Node::create_button().0;
    let b = Node::create_button().0;

    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 0);
    gtk_dom::layout::insert_child_at(root.as_node(), b.as_node(), 1);
    assert_eq!(child_count(&root), 2);

    // Re-insert `a` at position 1 — should reorder, not duplicate.
    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 1);
    assert_eq!(
        child_count(&root),
        2,
        "insert_child_at duplicated the parent->child edge"
    );

    // Order should be [b, a] now.
    let a_id = a.as_node().id();
    let b_id = b.as_node().id();
    assert_eq!(
        layout::children(root.as_node().id()),
        [b_id, a_id],
        "child order wrong after reorder"
    );
}

fn reorder_cascade_does_not_duplicate_edges() {
    let root = Node::create_vstack();

    let a = Node::create_button().0;
    let b = Node::create_button().0;
    let c = Node::create_button().0;
    gtk_dom::layout::attach_child(root.as_node(), a.as_node());
    gtk_dom::layout::attach_child(root.as_node(), b.as_node());
    gtk_dom::layout::attach_child(root.as_node(), c.as_node());
    assert_eq!(child_count(&root), 3);

    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 2);
    gtk_dom::layout::attach_child(root.as_node(), b.as_node());
    gtk_dom::layout::attach_child(root.as_node(), c.as_node());

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
