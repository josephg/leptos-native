//! Tests for the new `Node` state machine on the GTK port
//! (`Unmounted` / `Mounted` / `MountedBorrowed`) — mirrors
//! `cocoa/dom/tests/node_lifecycle.rs`.
//!
//! GTK is simpler than cocoa here: no per-Node handler bundle (signal
//! handlers are owned by the gtk widget's `connect_*` connection), so
//! we don't have the text-field-delegate explicit-drop dance to test.
//! The state-machine itself behaves identically.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{
    layout::{self, register_in_tree},
    node::NodeKind,
    Element, Node,
};

// =====================================================================
// 1. tree_id / mounted_handle reflect state transitions
// =====================================================================

fn unmounted_node_has_no_tree_id() {
    let el = Element::create("button");
    assert!(
        el.as_node().tree_id().is_none(),
        "fresh Node should be Unmounted"
    );
    assert!(el.as_node().mounted_handle().is_none());
}

fn register_transitions_to_mounted() {
    let el = Element::create("button");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id = el
        .as_node()
        .tree_id()
        .expect("mounted Node has tree_id")
        .1;
    assert!(
        tree.style(id).is_some(),
        "arena entry exists for mounted node"
    );
}

fn double_register_is_idempotent() {
    let el = Element::create("button");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id1 = el.as_node().tree_id().unwrap().1;
    register_in_tree(el.as_node(), &tree);
    let id2 = el.as_node().tree_id().unwrap().1;
    assert_eq!(id1, id2, "second register should not allocate a new id");
}

// =====================================================================
// 2. Style accessors route correctly per state
// =====================================================================

fn style_set_premount_survives_mount() {
    let el = Element::create("view");
    el.as_node().with_style_mut(|s| s.flex_grow = 3.5);

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(
        tree.style(id).unwrap().flex_grow,
        3.5,
        "premount style mutation must migrate into the arena"
    );
    el.as_node().with_style(|s| assert_eq!(s.flex_grow, 3.5));
}

fn style_set_postmount_lands_in_arena() {
    let el = Element::create("view");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id = el.as_node().tree_id().unwrap().1;

    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);

    assert_eq!(
        tree.style(id).unwrap().flex_grow,
        7.0,
        "post-mount style mutation must reach the arena"
    );
}

// =====================================================================
// 3. Drop ordering: arena entry goes away when last Node clone drops
// =====================================================================

fn dropping_last_node_clone_removes_arena_entry() {
    let tree = layout::new_tree();
    let id = {
        let el = Element::create("button");
        register_in_tree(el.as_node(), &tree);
        el.as_node().tree_id().unwrap().1
    };
    assert!(
        tree.style(id).is_none(),
        "arena entry must be removed when last Node clone drops"
    );
}

fn cloning_node_extends_lifetime() {
    let tree = layout::new_tree();
    let el = Element::create("button");
    register_in_tree(el.as_node(), &tree);
    let id = el.as_node().tree_id().unwrap().1;

    let clone = el.as_node().clone();
    drop(el);
    assert!(
        tree.style(id).is_some(),
        "arena entry must persist while a Node clone is alive"
    );

    drop(clone);
    assert!(
        tree.style(id).is_none(),
        "arena entry must drop after last clone goes away"
    );
}

// =====================================================================
// 4. MountedBorrowed: from_widget_with_handle does NOT remove arena
//    entries when dropped.
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let tree = layout::new_tree();

    let owner = Element::create("vstack");
    register_in_tree(owner.as_node(), &tree);
    let id = owner.as_node().tree_id().unwrap().1;

    let handle = owner.as_node().mounted_handle().unwrap();
    // Re-wrap the owner's widget under a MountedBorrowed Node.
    let widget = owner.as_node().widget().clone();
    let borrowed = Node::from_widget_with_handle(
        widget,
        NodeKind::Element,
        handle,
    );
    assert!(borrowed.tree_id().is_some(), "borrowed Node is Mounted-shaped");

    drop(borrowed);
    assert!(
        tree.style(id).is_some(),
        "arena entry must survive a MountedBorrowed Node drop"
    );

    drop(owner);
    assert!(
        tree.style(id).is_none(),
        "owning Node drop should now remove the entry"
    );
}

// =====================================================================
// 5. Widget identity stable across mount transition
// =====================================================================

fn widget_pointer_stable_through_mount() {
    use gtk4::glib::translate::ToGlibPtr;
    let el = Element::create("button");
    let ptr_before: *mut gtk4::ffi::GtkWidget =
        el.as_node().widget().to_glib_none().0;

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    let ptr_after: *mut gtk4::ffi::GtkWidget =
        el.as_node().widget().to_glib_none().0;
    assert_eq!(
        ptr_before, ptr_after,
        "widget() must return the same underlying pointer before and after mount"
    );
}

// =====================================================================
// Runner
// =====================================================================

fn main() {
    common::run_tests(&[
        ("unmounted_node_has_no_tree_id", unmounted_node_has_no_tree_id),
        ("register_transitions_to_mounted", register_transitions_to_mounted),
        ("double_register_is_idempotent", double_register_is_idempotent),
        ("style_set_premount_survives_mount", style_set_premount_survives_mount),
        ("style_set_postmount_lands_in_arena", style_set_postmount_lands_in_arena),
        ("dropping_last_node_clone_removes_arena_entry", dropping_last_node_clone_removes_arena_entry),
        ("cloning_node_extends_lifetime", cloning_node_extends_lifetime),
        ("borrowed_node_drop_does_not_remove_arena_entry", borrowed_node_drop_does_not_remove_arena_entry),
        ("widget_pointer_stable_through_mount", widget_pointer_stable_through_mount),
    ]);
}
