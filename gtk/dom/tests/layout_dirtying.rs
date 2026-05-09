//! mark_dirty discipline tests — every mutation must invalidate the
//! Taffy cache so re-layout sees fresh measurements.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{
    layout::{compute_layout, register_in_tree, TreeRef},
    Element,
};

fn fresh_tree() -> TreeRef {
    gtk_dom::layout::new_tree()
}

fn dirty_for(tree: &TreeRef, el: &Element) -> bool {
    let lh = el
        .as_node()
        .layout_slot()
        .borrow()
        .handle
        .clone()
        .expect("element has no LayoutHandle — wasn't registered");
    tree.tree.borrow().dirty(lh.node_id).unwrap_or(true)
}

/// After `compute_layout`, the root's dirty bit is cleared.
fn baseline_compute_clears_dirty() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);

    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&tree, &root), "root still dirty after compute");
}

fn attach_child_marks_parent_dirty() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    let child = Element::create("button");
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after attach_child"
    );
}

fn detach_child_marks_parent_dirty() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    let child = Element::create("button");
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    gtk_dom::layout::detach_child(root.as_node(), child.as_node());

    assert!(
        dirty_for(&tree, &root),
        "parent not marked dirty after detach_child"
    );
}

fn set_text_marks_node_dirty() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    let child = Element::create("label");
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&tree, &child));

    child.set_attribute("value", "now I have content");

    assert!(
        dirty_for(&tree, &child),
        "label not marked dirty after text change"
    );
}

fn set_style_width_marks_node_dirty() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    compute_layout(root.as_node(), (200.0, 200.0));
    assert!(!dirty_for(&tree, &root));

    gtk_dom::layout::set_width(root.as_node(), 150.0);

    assert!(
        dirty_for(&tree, &root),
        "node not marked dirty after set_width"
    );
}

// ---------------------------------------------------------------------
// Idempotency: re-attaching an already-attached child must not produce
// a duplicate parent->child edge in the Taffy tree.
// ---------------------------------------------------------------------

fn child_count(tree: &TreeRef, parent: &Element) -> usize {
    let lh = parent
        .as_node()
        .layout_slot()
        .borrow()
        .handle
        .clone()
        .expect("element has no LayoutHandle");
    tree.tree
        .borrow()
        .children(lh.node_id)
        .map(|c| c.len())
        .unwrap_or(0)
}

fn attach_child_is_idempotent() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    let child = Element::create("button");

    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(child_count(&tree, &root), 1);
    gtk_dom::layout::attach_child(root.as_node(), child.as_node());
    assert_eq!(
        child_count(&tree, &root),
        1,
        "attach_child duplicated the parent->child edge"
    );
}

fn insert_child_at_is_idempotent() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);
    let a = Element::create("button");
    let b = Element::create("button");

    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 0);
    gtk_dom::layout::insert_child_at(root.as_node(), b.as_node(), 1);
    assert_eq!(child_count(&tree, &root), 2);

    // Re-insert `a` at position 1 — should reorder, not duplicate.
    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 1);
    assert_eq!(
        child_count(&tree, &root),
        2,
        "insert_child_at duplicated the parent->child edge"
    );

    // Order should be [b, a] now.
    let lh = root.as_node().layout_slot().borrow().handle.clone().unwrap();
    let kids = tree.tree.borrow().children(lh.node_id).unwrap();
    let a_id = a.as_node().layout_slot().borrow().handle.clone().unwrap().node_id;
    let b_id = b.as_node().layout_slot().borrow().handle.clone().unwrap().node_id;
    assert_eq!(kids, vec![b_id, a_id], "child order wrong after reorder");
}

fn reorder_cascade_does_not_duplicate_edges() {
    let tree = fresh_tree();
    let root = Element::create("vstack");
    register_in_tree(root.as_node(), &tree);

    let a = Element::create("button");
    let b = Element::create("button");
    let c = Element::create("button");
    gtk_dom::layout::attach_child(root.as_node(), a.as_node());
    gtk_dom::layout::attach_child(root.as_node(), b.as_node());
    gtk_dom::layout::attach_child(root.as_node(), c.as_node());
    assert_eq!(child_count(&tree, &root), 3);

    gtk_dom::layout::insert_child_at(root.as_node(), a.as_node(), 2);
    gtk_dom::layout::attach_child(root.as_node(), b.as_node());
    gtk_dom::layout::attach_child(root.as_node(), c.as_node());

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
