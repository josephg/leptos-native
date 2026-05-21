//! mark_dirty discipline tests — every mutation must invalidate the
//! Taffy cache so re-layout sees fresh measurements.

#![cfg(feature = "gtk")]

mod common;


use leptos_gtk::dom::GtkNode;
use leptos_gtk::dom::layout::{self, GtkBackend};
use renderer::LayoutBackend;

fn dirty_for(el: &GtkNode) -> bool {
    GtkBackend::dirty(el.as_node().id())
}

/// After `compute_layout`, the root's dirty bit is cleared.
fn baseline_compute_clears_dirty() {
    let root = GtkNode::create_vstack();

    layout::compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root), "root still dirty after compute");
}

fn attach_child_marks_parent_dirty() {
    let root = GtkNode::create_vstack();
    layout::compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    let child = GtkNode::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&root),
        "parent not marked dirty after attach_child"
    );
}

fn detach_child_marks_parent_dirty() {
    let root = GtkNode::create_vstack();
    let child = GtkNode::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());
    layout::compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    layout::detach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&root),
        "parent not marked dirty after detach_child"
    );
}

fn set_text_marks_node_dirty() {
    let root = GtkNode::create_vstack();
    let child = GtkNode::create_label().0;
    layout::attach_child(root.as_node(), child.as_node());
    layout::compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&child));

    child.set_value("now I have content");

    assert!(
        dirty_for(&child),
        "label not marked dirty after text change"
    );
}

fn set_style_width_marks_node_dirty() {
    let root = GtkNode::create_vstack();
    layout::compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&root));

    layout::set_width(root.as_node(), 150.0);

    assert!(
        dirty_for(&root),
        "node not marked dirty after set_width"
    );
}

// ---------------------------------------------------------------------
// Idempotency: re-attaching an already-attached child must not produce
// a duplicate parent->child edge in the Taffy tree.
// ---------------------------------------------------------------------

fn child_count(parent: &GtkNode) -> usize {
    GtkBackend::children(parent.as_node().id()).len()
}

fn attach_child_is_idempotent() {
    let root = GtkNode::create_vstack();
    let child = GtkNode::create_button().0;

    layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(child_count(&root), 1);
    layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(
        child_count(&root),
        1,
        "attach_child duplicated the parent->child edge"
    );
}

fn insert_child_at_is_idempotent() {
    let root = GtkNode::create_vstack();
    let a = GtkNode::create_button().0;
    let b = GtkNode::create_button().0;

    layout::insert_child_at(root.as_node(), a.as_node(), 0);
    layout::insert_child_at(root.as_node(), b.as_node(), 1);
    assert_eq!(child_count(&root), 2);

    // Re-insert `a` at position 1 — should reorder, not duplicate.
    layout::insert_child_at(root.as_node(), a.as_node(), 1);
    assert_eq!(
        child_count(&root),
        2,
        "insert_child_at duplicated the parent->child edge"
    );

    // Order should be [b, a] now.
    let a_id = a.as_node().id();
    let b_id = b.as_node().id();
    assert_eq!(
        GtkBackend::children(root.as_node().id()),
        [b_id, a_id],
        "child order wrong after reorder"
    );
}

fn reorder_cascade_does_not_duplicate_edges() {
    let root = GtkNode::create_vstack();

    let a = GtkNode::create_button().0;
    let b = GtkNode::create_button().0;
    let c = GtkNode::create_button().0;
    layout::attach_child(root.as_node(), a.as_node());
    layout::attach_child(root.as_node(), b.as_node());
    layout::attach_child(root.as_node(), c.as_node());
    assert_eq!(child_count(&root), 3);

    layout::insert_child_at(root.as_node(), a.as_node(), 2);
    layout::attach_child(root.as_node(), b.as_node());
    layout::attach_child(root.as_node(), c.as_node());

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
