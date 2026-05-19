//! Tests for the `Node` lifecycle on the GTK port: arena allocation,
//! drop semantics, borrowed wrappers, and the refcount /
//! parent-reachability removal rule. Mirror of
//! `cocoa/dom/tests/node_lifecycle.rs`.
//!
//! After the Phase 3 refactor, every Node is eagerly allocated in a
//! `LayoutTree` from creation — there's no Unmounted/Mounted state
//! machine anymore. GTK is simpler than cocoa here: no per-Node
//! handler bundle (signal handlers are owned by the gtk widget's
//! `connect_*` connection), so we skip the handler-store regression
//! tests.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{layout, Element, Node};

// =====================================================================
// 1. Fresh nodes are in their tree from creation
// =====================================================================

fn freshly_created_node_has_tree_id() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let (_, id) = el
        .as_node()
        .tree_id()
        .expect("fresh Node has tree_id");
    assert!(
        tree.style(id).is_some(),
        "arena entry exists for freshly-created node"
    );
}

// =====================================================================
// 2. Style accessors route through the arena
// =====================================================================

fn style_mutation_lands_in_arena() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "view");
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(tree.style(id).unwrap().flex_grow, 7.0);
}

// =====================================================================
// 3. Drop ordering: arena entry goes away when last Node clone drops
// =====================================================================

fn dropping_last_node_clone_removes_arena_entry() {
    let tree = layout::new_tree();
    let id = {
        let el = Element::create(&tree, "button");
        el.as_node().tree_id().unwrap().1
        // `el` drops here.
    };
    assert!(
        tree.style(id).is_none(),
        "arena entry must be removed when last Node clone drops"
    );
}

fn cloning_node_extends_lifetime() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
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
// 4. Borrowed wrapper: from_widget_with_handle Node does NOT remove
//    arena entries when dropped.
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let tree = layout::new_tree();

    let owner = Element::create(&tree, "vstack");
    let id = owner.as_node().tree_id().unwrap().1;

    let handle = owner.as_node().mounted_handle().unwrap();
    let widget = owner.as_node().widget().clone();
    let borrowed = Node::from_widget_with_handle(widget, handle);
    assert!(borrowed.tree_id().is_some(), "borrowed Node has tree_id");

    drop(borrowed);
    assert!(
        tree.style(id).is_some(),
        "arena entry must survive a borrowed-Node drop"
    );

    drop(owner);
    assert!(
        tree.style(id).is_none(),
        "owning Node drop should now remove the entry"
    );
}

// =====================================================================
// 5. Refcount + parent-reachability removal rule (Phase 1 refactor)
// =====================================================================

fn new_leaf_starts_at_refcount_one() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(
        tree.refcount_for_test(id),
        Some(1),
        "newly-created node has refcount=1 (the caller's handle)"
    );
}

fn incref_increments_refcount() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let id = el.as_node().tree_id().unwrap().1;
    tree.incref(id);
    assert_eq!(tree.refcount_for_test(id), Some(2));
    tree.incref(id);
    assert_eq!(tree.refcount_for_test(id), Some(3));
    // Decref back to 1 so the eventual Node drop doesn't underflow.
    tree.decref(id);
    tree.decref(id);
}

fn decref_decrements_but_keeps_alive_if_attached() {
    let tree = layout::new_tree();
    let root = Element::create(&tree, "vstack");
    let child = Element::create(&tree, "button");
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;
    assert_eq!(tree.refcount_for_test(child_id), Some(1));

    tree.decref(child_id);
    assert_eq!(
        tree.refcount_for_test(child_id),
        Some(0),
        "decref drops count to 0"
    );
    assert!(
        tree.style(child_id).is_some(),
        "attached entry must NOT be removed at refcount=0"
    );

    // Re-incref so the implicit child drop doesn't underflow.
    tree.incref(child_id);
}

fn detached_orphan_with_refcount_zero_is_removed() {
    let tree = layout::new_tree();
    let root = Element::create(&tree, "vstack");
    let child = Element::create(&tree, "button");
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;

    tree.decref(child_id);
    assert!(tree.style(child_id).is_some());

    layout::detach_child(root.as_node(), child.as_node());
    assert!(
        tree.style(child_id).is_none(),
        "detached entry with refcount=0 must be removed (reachability GC)"
    );
}

fn detached_orphan_with_handles_stays() {
    let tree = layout::new_tree();
    let root = Element::create(&tree, "vstack");
    let child = Element::create(&tree, "button");
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;
    layout::detach_child(root.as_node(), child.as_node());
    assert!(
        tree.style(child_id).is_some(),
        "detached entry with refcount > 0 must stay (Node handle keeps it alive)"
    );
}

fn decref_below_zero_is_safe() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let id = el.as_node().tree_id().unwrap().1;

    tree.decref(id);
    assert!(tree.style(id).is_none());
    tree.decref(id); // no panic
}

fn decref_on_nonexistent_is_noop() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let id = el.as_node().tree_id().unwrap().1;
    tree.remove(id);
    tree.decref(id); // no panic
    tree.incref(id); // no panic
}

// =====================================================================
// 6. Widget identity stable across repeated accesses
// =====================================================================

fn widget_pointer_stable() {
    use gtk4::glib::translate::ToGlibPtr;
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let ptr_before: *mut gtk4::ffi::GtkWidget =
        el.as_node().widget().to_glib_none().0;
    let ptr_after: *mut gtk4::ffi::GtkWidget =
        el.as_node().widget().to_glib_none().0;
    assert_eq!(ptr_before, ptr_after, "widget() pointer must be stable");
}

// =====================================================================
// 7. WeakNode / WeakElement — cycle-safe closure capture (Phase 4)
// =====================================================================

fn weak_node_upgrades_while_node_alive() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let weak = el.as_node().downgrade();

    assert!(weak.is_alive(), "weak handle is alive while Node is");
    let strong = weak.upgrade().expect("upgrade succeeds");
    assert!(strong.ptr_eq(el.as_node()), "upgrade returns the same Node");
}

fn weak_node_upgrade_fails_after_drop() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let weak = el.as_node().downgrade();
    drop(el);

    assert!(!weak.is_alive(), "weak handle is dead after Element drops");
    assert!(weak.upgrade().is_none(), "upgrade returns None");
}

fn weak_element_upgrade_round_trips() {
    let tree = layout::new_tree();
    let el = Element::create(&tree, "button");
    let weak = el.weak();
    let recovered = weak.upgrade().expect("alive");
    assert!(recovered.as_node().ptr_eq(el.as_node()));
}

// =====================================================================
// Runner
// =====================================================================

fn main() {
    common::run_tests(&[
        ("freshly_created_node_has_tree_id", freshly_created_node_has_tree_id),
        ("style_mutation_lands_in_arena", style_mutation_lands_in_arena),
        ("dropping_last_node_clone_removes_arena_entry", dropping_last_node_clone_removes_arena_entry),
        ("cloning_node_extends_lifetime", cloning_node_extends_lifetime),
        ("borrowed_node_drop_does_not_remove_arena_entry", borrowed_node_drop_does_not_remove_arena_entry),
        ("new_leaf_starts_at_refcount_one", new_leaf_starts_at_refcount_one),
        ("incref_increments_refcount", incref_increments_refcount),
        ("decref_decrements_but_keeps_alive_if_attached", decref_decrements_but_keeps_alive_if_attached),
        ("detached_orphan_with_refcount_zero_is_removed", detached_orphan_with_refcount_zero_is_removed),
        ("detached_orphan_with_handles_stays", detached_orphan_with_handles_stays),
        ("decref_below_zero_is_safe", decref_below_zero_is_safe),
        ("decref_on_nonexistent_is_noop", decref_on_nonexistent_is_noop),
        ("widget_pointer_stable", widget_pointer_stable),
        ("weak_node_upgrades_while_node_alive", weak_node_upgrades_while_node_alive),
        ("weak_node_upgrade_fails_after_drop", weak_node_upgrade_fails_after_drop),
        ("weak_element_upgrade_round_trips", weak_element_upgrade_round_trips),
    ]);
}
