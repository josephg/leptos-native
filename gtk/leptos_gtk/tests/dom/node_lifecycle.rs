//! Tests for the `Node` lifecycle under the thread-local store (GTK).
//!
//! A `Node` is a `Copy` `NodeId` into the per-thread store. There is
//! no refcount and no drop-driven removal: a node is created
//! Unattached, becomes Attached via `add_child`, and is Freed only by
//! an explicit `teardown` / `remove` (which cascades to the structural
//! subtree). Stale ids resolve to `None`/no-op via the generational
//! slotmap key. GTK is simpler than cocoa — no per-node handler bundle.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::dom::{layout, GtkNode, layout::GtkBackend};
use renderer::LayoutBackend;

// 1. Fresh nodes are in the store from creation.
fn freshly_created_node_is_in_store() {
    let el = GtkNode::create_button().0;
    assert!(
        GtkBackend::style(el.id()).is_some(),
        "store entry exists for a freshly-created node"
    );
}

// 2. Style accessors route through the store.
fn style_mutation_lands_in_store() {
    let el = GtkNode::create_stack();
    el.with_style_mut(|s| s.flex_grow = 7.0);
    assert_eq!(GtkBackend::style(el.id()).unwrap().flex_grow, 7.0);
}

// 3. Explicit teardown removes the entry.
fn teardown_removes_store_entry() {
    let el = GtkNode::create_button().0;
    let id = el.id();
    assert!(GtkBackend::style(id).is_some());
    el.teardown();
    assert!(GtkBackend::style(id).is_none(), "teardown removes the entry");
}

// 5. teardown cascades to structural children.
fn teardown_cascades_to_children() {
    let root = GtkNode::create_vstack();
    let child = GtkNode::create_button().0;
    layout::attach_child(root, child);
    let root_id = root.id();
    let child_id = child.id();
    assert!(GtkBackend::style(root_id).is_some());
    assert!(GtkBackend::style(child_id).is_some());

    root.teardown();
    assert!(GtkBackend::style(root_id).is_none(), "root removed");
    assert!(
        GtkBackend::style(child_id).is_none(),
        "structural child removed by the teardown cascade"
    );
}

// 6. Detaching does not free.
fn detach_does_not_free() {
    let root = GtkNode::create_vstack();
    let child = GtkNode::create_button().0;
    layout::attach_child(root, child);
    let child_id = child.id();

    layout::detach_child(root, child);
    assert!(
        GtkBackend::style(child_id).is_some(),
        "detach leaves the node Unattached but present"
    );
    assert_eq!(GtkBackend::parent(child_id), None);

    child.teardown();
    assert!(GtkBackend::style(child_id).is_none());
}

// 7. Stale ids are safe no-ops.
fn stale_id_accessors_are_safe() {
    let el = GtkNode::create_button().0;
    let id = el.id();
    GtkBackend::remove(id);
    assert!(GtkBackend::style(id).is_none());
    assert!(GtkBackend::children(id).is_empty());
    assert_eq!(GtkBackend::parent(id), None);
    GtkBackend::remove(id); // double-remove is a no-op
}

// 8. Widget identity stable across repeated accesses.
fn widget_pointer_stable() {
    use gtk4::prelude::*;
    let el = GtkNode::create_button().0;
    let p1 = el.widget().as_ptr();
    let p2 = el.widget().as_ptr();
    assert_eq!(p1, p2, "widget() pointer must be stable");
}

// 10. A mounted subtree returns the store to baseline after the
// root is torn down — the headless analogue of whole-window
// teardown (opening a real gtk::ApplicationWindow needs an app +
// main loop). Locks in the explicit-free lifecycle the
// `ElementState::Drop` safety net relies on.
fn subtree_teardown_returns_to_baseline() {
    let baseline = GtkBackend::node_count();

    let root = GtkNode::create_vstack();
    let row = GtkNode::create_stack();
    let b1 = GtkNode::create_button().0;
    let b2 = GtkNode::create_button().0;
    let label = GtkNode::create_label().0;
    layout::attach_child(row, b1);
    layout::attach_child(row, b2);
    layout::attach_child(root, label);
    layout::attach_child(root, row);

    assert!(GtkBackend::node_count() > baseline, "mounting grows the store");

    root.teardown();
    assert_eq!(
        GtkBackend::node_count(),
        baseline,
        "store returned to baseline after subtree teardown — no leak"
    );
}

// 11. An unattached (orphaned) node is fully freed by teardown.
fn unattached_node_teardown_returns_to_baseline() {
    let baseline = GtkBackend::node_count();
    let el = GtkNode::create_button().0;
    assert_eq!(GtkBackend::node_count(), baseline + 1);
    el.teardown();
    assert_eq!(
        GtkBackend::node_count(),
        baseline,
        "unattached node freed by teardown — no orphan leak"
    );
}

fn main() {
    common::run_tests(&[
        ("freshly_created_node_is_in_store", freshly_created_node_is_in_store),
        ("style_mutation_lands_in_store", style_mutation_lands_in_store),
        ("teardown_removes_store_entry", teardown_removes_store_entry),
        ("teardown_cascades_to_children", teardown_cascades_to_children),
        ("detach_does_not_free", detach_does_not_free),
        ("stale_id_accessors_are_safe", stale_id_accessors_are_safe),
        ("widget_pointer_stable", widget_pointer_stable),
        ("subtree_teardown_returns_to_baseline", subtree_teardown_returns_to_baseline),
        ("unattached_node_teardown_returns_to_baseline", unattached_node_teardown_returns_to_baseline),
    ]);
}
