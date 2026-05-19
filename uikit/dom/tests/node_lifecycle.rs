//! Tests for the `Node` lifecycle on the iOS port: arena allocation,
//! drop semantics, borrowed wrappers, handler ownership, and the
//! refcount / parent-reachability removal rule. Mirror of
//! `cocoa/dom/tests/node_lifecycle.rs`.
//!
//! After the Phase 3 refactor, every Node is eagerly allocated in a
//! `LayoutTree` from creation — there's no Unmounted/Mounted state
//! machine anymore.

#![cfg(target_os = "ios")]

mod common;

use ios_dom::{
    event::{handler_store_size_for_test, text_view_store_size_for_test},
    layout,
    Element, Node, NodeKind,
};

// =====================================================================
// 1. Fresh nodes are in their tree from creation
// =====================================================================

fn freshly_created_node_has_tree_id() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
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
// 2. Style/meta accessors route through the arena
// =====================================================================

fn style_mutation_lands_in_arena() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_vstack(&tree);
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(tree.style(id).unwrap().flex_grow, 7.0);
}

fn scroll_view_has_is_scroll_view_at_create_time() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_scroll_view(&tree).0;
    let meta = el.as_node().with_meta(|m| m.clone());
    assert!(meta.is_scroll_view, "scroll_view sets is_scroll_view");
}

// =====================================================================
// 3. Drop ordering: arena entry goes away when last Node clone drops
// =====================================================================

fn dropping_last_node_clone_removes_arena_entry() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let id = {
        let el = Element::create_button(&tree).0;
        el.as_node().tree_id().unwrap().1
    };
    assert!(
        tree.style(id).is_none(),
        "arena entry must be removed when last Node clone drops"
    );
}

fn cloning_node_extends_lifetime() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
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
// 4. Borrowed wrapper: from_view_with_handle Node does NOT remove
//    arena entries when dropped.
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();

    let owner = Element::create_vstack(&tree);
    let id = owner.as_node().tree_id().unwrap().1;

    let handle = owner.as_node().mounted_handle().unwrap();
    let view_retained = owner.as_node().ui_view_retained();
    let borrowed = Node::from_view_with_handle(
        view_retained,
        NodeKind::Element,
        handle,
    );
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
// 5. Handler-lifecycle: action-target install + drop balance
// =====================================================================

fn handler_installed_drops_with_node() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    {
        let tree = layout::new_tree();
        let el = Element::create_button(&tree).0;
        el.on_click(|| {});
        assert_eq!(
            handler_store_size_for_test(),
            baseline + 1,
            "on_click should allocate one ActionTarget"
        );
    }
    assert_eq!(
        handler_store_size_for_test(),
        baseline,
        "ActionTarget should dealloc when Node + tree go away"
    );
}

// =====================================================================
// 6. Text-view delegate cleanup: the explicit-drop fix in
//    IosNodeHandlers::Drop should ensure delegates dealloc cleanly.
// =====================================================================

fn text_view_delegate_releases_on_node_drop() {
    let _mtm = common::test_mtm();
    let baseline = text_view_store_size_for_test();
    {
        let tree = layout::new_tree();
        let el = Element::create_text_view(&tree).0;
        el.on_text_view_change(|_| {});
        assert_eq!(
            text_view_store_size_for_test(),
            baseline + 1,
            "ensure_text_view_entry should allocate one TextViewDelegate"
        );
    }
    assert_eq!(
        text_view_store_size_for_test(),
        baseline,
        "TextViewDelegate must dealloc when the Node drops — \
         regression test for the explicit-drop fix in \
         IosNodeHandlers::drop"
    );
}

// =====================================================================
// 7. Refcount + parent-reachability removal rule (Phase 1 refactor)
// =====================================================================

fn new_leaf_starts_at_refcount_one() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(
        tree.refcount_for_test(id),
        Some(1),
        "newly-created node has refcount=1 (the caller's handle)"
    );
}

fn incref_increments_refcount() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let id = el.as_node().tree_id().unwrap().1;
    tree.incref(id);
    assert_eq!(tree.refcount_for_test(id), Some(2));
    tree.incref(id);
    assert_eq!(tree.refcount_for_test(id), Some(3));
    tree.decref(id);
    tree.decref(id);
}

fn decref_decrements_but_keeps_alive_if_attached() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_vstack(&tree);
    let child = Element::create_button(&tree).0;
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

    tree.incref(child_id);
}

fn detached_orphan_with_refcount_zero_is_removed() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_vstack(&tree);
    let child = Element::create_button(&tree).0;
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
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_vstack(&tree);
    let child = Element::create_button(&tree).0;
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;
    layout::detach_child(root.as_node(), child.as_node());
    assert!(
        tree.style(child_id).is_some(),
        "detached entry with refcount > 0 must stay (Node handle keeps it alive)"
    );
}

fn decref_below_zero_is_safe() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let id = el.as_node().tree_id().unwrap().1;

    tree.decref(id);
    assert!(tree.style(id).is_none());
    tree.decref(id);
}

fn decref_on_nonexistent_is_noop() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let id = el.as_node().tree_id().unwrap().1;
    tree.remove(id);
    tree.decref(id);
    tree.incref(id);
}

// =====================================================================
// 8. UIView identity stable across repeated accesses
// =====================================================================

fn ui_view_pointer_stable() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let ptr_before: *const objc2_ui_kit::UIView = el.as_node().ui_view();
    let ptr_after: *const objc2_ui_kit::UIView = el.as_node().ui_view();
    assert_eq!(ptr_before, ptr_after, "ui_view() pointer must be stable");
}

// =====================================================================
// 9. WeakNode / WeakElement — cycle-safe closure capture (Phase 4)
// =====================================================================

fn weak_node_upgrades_while_node_alive() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let weak = el.as_node().downgrade();

    assert!(weak.is_alive(), "weak handle is alive while Node is");
    let strong = weak.upgrade().expect("upgrade succeeds");
    assert!(strong.ptr_eq(el.as_node()), "upgrade returns the same Node");
}

fn weak_node_upgrade_fails_after_drop() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let weak = el.as_node().downgrade();
    drop(el);

    assert!(!weak.is_alive(), "weak handle is dead after Element drops");
    assert!(weak.upgrade().is_none(), "upgrade returns None");
}

fn weak_element_round_trips_kind() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let weak = el.weak();
    let recovered = weak.upgrade().expect("alive");
    assert_eq!(recovered.as_node().kind(), ios_dom::NodeKind::Element);
}

fn closure_capturing_weak_element_does_not_keep_arena_alive() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let tree = layout::new_tree();
    let id = {
        let el = Element::create_button(&tree).0;
        let id = el.as_node().tree_id().unwrap().1;

        let weak = el.weak();
        el.on_click(move || {
            if let Some(_e) = weak.upgrade() {
                // would re-enter the element here in real code
            }
        });

        assert_eq!(
            handler_store_size_for_test(),
            baseline + 1,
            "on_click allocated an ActionTarget"
        );
        id
    };

    assert!(
        tree.style(id).is_none(),
        "WeakElement in handler closure does NOT prevent arena cleanup"
    );
    assert_eq!(
        handler_store_size_for_test(),
        baseline,
        "ActionTarget must release when arena entry drops"
    );
}

// =====================================================================
// Runner
// =====================================================================

fn main() {
    common::run_tests(&[
        ("freshly_created_node_has_tree_id", freshly_created_node_has_tree_id),
        ("style_mutation_lands_in_arena", style_mutation_lands_in_arena),
        ("scroll_view_has_is_scroll_view_at_create_time", scroll_view_has_is_scroll_view_at_create_time),
        ("dropping_last_node_clone_removes_arena_entry", dropping_last_node_clone_removes_arena_entry),
        ("cloning_node_extends_lifetime", cloning_node_extends_lifetime),
        ("borrowed_node_drop_does_not_remove_arena_entry", borrowed_node_drop_does_not_remove_arena_entry),
        ("handler_installed_drops_with_node", handler_installed_drops_with_node),
        ("text_view_delegate_releases_on_node_drop", text_view_delegate_releases_on_node_drop),
        ("new_leaf_starts_at_refcount_one", new_leaf_starts_at_refcount_one),
        ("incref_increments_refcount", incref_increments_refcount),
        ("decref_decrements_but_keeps_alive_if_attached", decref_decrements_but_keeps_alive_if_attached),
        ("detached_orphan_with_refcount_zero_is_removed", detached_orphan_with_refcount_zero_is_removed),
        ("detached_orphan_with_handles_stays", detached_orphan_with_handles_stays),
        ("decref_below_zero_is_safe", decref_below_zero_is_safe),
        ("decref_on_nonexistent_is_noop", decref_on_nonexistent_is_noop),
        ("ui_view_pointer_stable", ui_view_pointer_stable),
        ("weak_node_upgrades_while_node_alive", weak_node_upgrades_while_node_alive),
        ("weak_node_upgrade_fails_after_drop", weak_node_upgrade_fails_after_drop),
        ("weak_element_round_trips_kind", weak_element_round_trips_kind),
        ("closure_capturing_weak_element_does_not_keep_arena_alive", closure_capturing_weak_element_does_not_keep_arena_alive),
    ]);
}
