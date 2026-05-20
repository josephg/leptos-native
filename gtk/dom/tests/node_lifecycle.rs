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

use gtk_dom::{layout, Node};

// 1. Fresh nodes are in the store from creation.
fn freshly_created_node_is_in_store() {
    let el = Node::create_button().0;
    assert!(
        layout::style(el.as_node().id()).is_some(),
        "store entry exists for a freshly-created node"
    );
}

// 2. Style accessors route through the store.
fn style_mutation_lands_in_store() {
    let el = Node::create_stack();
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);
    assert_eq!(layout::style(el.as_node().id()).unwrap().flex_grow, 7.0);
}

// 3. Explicit teardown removes the entry.
fn teardown_removes_store_entry() {
    let el = Node::create_button().0;
    let id = el.as_node().id();
    assert!(layout::style(id).is_some());
    el.as_node().teardown();
    assert!(layout::style(id).is_none(), "teardown removes the entry");
}

// 4. A Node copy is a non-owning weak handle.
fn copying_node_id_does_not_affect_lifetime() {
    let el = Node::create_button().0;
    let id = el.as_node().id();
    let copy = *el.as_node();
    let _ = copy;
    assert!(
        layout::style(id).is_some(),
        "a dropped Node copy must not remove the entry"
    );
    el.as_node().teardown();
    assert!(layout::style(id).is_none());
}

// 5. teardown cascades to structural children.
fn teardown_cascades_to_children() {
    let root = Node::create_vstack();
    let child = Node::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());
    let root_id = root.as_node().id();
    let child_id = child.as_node().id();
    assert!(layout::style(root_id).is_some());
    assert!(layout::style(child_id).is_some());

    root.as_node().teardown();
    assert!(layout::style(root_id).is_none(), "root removed");
    assert!(
        layout::style(child_id).is_none(),
        "structural child removed by the teardown cascade"
    );
}

// 6. Detaching does not free.
fn detach_does_not_free() {
    let root = Node::create_vstack();
    let child = Node::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());
    let child_id = child.as_node().id();

    layout::detach_child(root.as_node(), child.as_node());
    assert!(
        layout::style(child_id).is_some(),
        "detach leaves the node Unattached but present"
    );
    assert_eq!(layout::parent(child_id), None);

    child.as_node().teardown();
    assert!(layout::style(child_id).is_none());
}

// 7. Stale ids are safe no-ops.
fn stale_id_accessors_are_safe() {
    let el = Node::create_button().0;
    let id = el.as_node().id();
    layout::remove(id);
    assert!(layout::style(id).is_none());
    assert!(layout::children(id).is_empty());
    assert_eq!(layout::parent(id), None);
    layout::remove(id); // double-remove is a no-op
}

// 8. Widget identity stable across repeated accesses.
fn widget_pointer_stable() {
    use gtk4::prelude::*;
    let el = Node::create_button().0;
    let p1 = el.as_node().widget().as_ptr();
    let p2 = el.as_node().widget().as_ptr();
    assert_eq!(p1, p2, "widget() pointer must be stable");
}

// 9. WeakNode — a Copy id that resolves only while present.
fn weak_node_upgrades_while_present() {
    let el = Node::create_button().0;
    let weak = el.as_node().downgrade();
    assert!(weak.is_alive());
    let strong = weak.upgrade().expect("upgrade succeeds");
    assert!(strong.ptr_eq(el.as_node()));
}

fn weak_node_upgrade_fails_after_teardown() {
    let el = Node::create_button().0;
    let weak = el.as_node().downgrade();
    el.as_node().teardown();
    assert!(!weak.is_alive());
    assert!(weak.upgrade().is_none());
}

fn main() {
    common::run_tests(&[
        ("freshly_created_node_is_in_store", freshly_created_node_is_in_store),
        ("style_mutation_lands_in_store", style_mutation_lands_in_store),
        ("teardown_removes_store_entry", teardown_removes_store_entry),
        ("copying_node_id_does_not_affect_lifetime", copying_node_id_does_not_affect_lifetime),
        ("teardown_cascades_to_children", teardown_cascades_to_children),
        ("detach_does_not_free", detach_does_not_free),
        ("stale_id_accessors_are_safe", stale_id_accessors_are_safe),
        ("widget_pointer_stable", widget_pointer_stable),
        ("weak_node_upgrades_while_present", weak_node_upgrades_while_present),
        ("weak_node_upgrade_fails_after_teardown", weak_node_upgrade_fails_after_teardown),
    ]);
}
