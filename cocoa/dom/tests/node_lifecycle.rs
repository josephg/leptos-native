//! Tests for the `Node` lifecycle: arena allocation, drop semantics,
//! borrowed wrappers, handler ownership, and the refcount / parent-
//! reachability removal rule.
//!
//! After the Phase 3 refactor, every Node is eagerly allocated in a
//! `LayoutTree` from creation — there's no Unmounted/Mounted state
//! machine anymore. These tests cover the simplified invariants.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{
    event::{handler_store_size_for_test, text_field_store_size_for_test},
    layout,
    Element, Node,
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
    let el = Element::create_container(&tree);
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(tree.style(id).unwrap().flex_grow, 7.0);
}

fn scroll_view_has_child_taffy_parent_at_create_time() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_scroll_view(&tree).0;
    let meta = el.as_node().with_meta(|m| m.clone());
    assert!(meta.is_scroll_view, "scroll_view sets is_scroll_view");
    assert!(
        meta.child_taffy_parent.is_some(),
        "scroll_view allocates documentView wrapper eagerly"
    );
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
        // `el` drops here.
    };
    // After the only Node clone drops, the arena entry should be gone.
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

    // Hold an extra clone, drop the original Element.
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
//    arena entries when dropped (so the "real" owner's entry stays).
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = layout::new_tree();

    let owner = Element::create_container_with(&tree, mtm);
    let id = owner.as_node().tree_id().unwrap().1;

    // Synthesise a borrowed wrapper using the owner's handle.
    let handle = owner.as_node().mounted_handle().unwrap();
    let view_retained = owner.as_node().ns_view_retained();
    let borrowed = Node::from_view_with_handle(view_retained, handle);
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
// 5. Handler-lifecycle: installing handler and dropping the Node
//    fires NodeHandlers::Drop (ActionTarget store decrements).
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
        // tree + el drop here.
    }
    assert_eq!(
        handler_store_size_for_test(),
        baseline,
        "ActionTarget should dealloc when Node + tree go away"
    );
}

// =====================================================================
// 6. Text-field delegate cleanup: the field-drop-order fix in
//    NodeHandlers::Drop should ensure delegates dealloc cleanly.
// =====================================================================

fn text_field_delegate_releases_on_node_drop() {
    let _mtm = common::test_mtm();
    let baseline = text_field_store_size_for_test();
    {
        let tree = layout::new_tree();
        let el = Element::create_text_field(&tree).0;
        el.on_text_change(|_| {});
        assert_eq!(
            text_field_store_size_for_test(),
            baseline + 1,
            "ensure_text_field_entry should allocate one TextFieldDelegate"
        );
    }
    assert_eq!(
        text_field_store_size_for_test(),
        baseline,
        "TextFieldDelegate must dealloc when the Node drops — \
         regression test for the field-drop-order fix in \
         NodeHandlers::drop"
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
    // Decref back to 1 so the eventual Node drop doesn't underflow.
    tree.decref(id);
    tree.decref(id);
}

fn decref_decrements_but_keeps_alive_if_attached() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_container(&tree);
    let child = Element::create_button(&tree).0;
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;
    assert_eq!(tree.refcount_for_test(child_id), Some(1));

    // Decref to 0. Child is still attached (parent = root), so the
    // entry stays alive under the parent-reachability rule.
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
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_container(&tree);
    let child = Element::create_button(&tree).0;
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;

    // Decref to 0 first — entry stays (parent attached).
    tree.decref(child_id);
    assert!(tree.style(child_id).is_some());

    // Now detach. With refcount=0 AND parent=None, the reachability
    // GC kicks in: tree.remove_child should drop the entry.
    layout::detach_child(root.as_node(), child.as_node());
    assert!(
        tree.style(child_id).is_none(),
        "detached entry with refcount=0 must be removed (reachability GC)"
    );
}

fn detached_orphan_with_handles_stays() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let root = Element::create_container(&tree);
    let child = Element::create_button(&tree).0;
    layout::attach_child(root.as_node(), child.as_node());

    let child_id = child.as_node().tree_id().unwrap().1;
    // Refcount stays at 1 (child's Node is still alive).
    // Detach. parent → None, but refcount > 0 — entry must stay.
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

    // Detach root state (parent already None) and decref twice. The
    // first decref hits 0 → removes the entry. The second is a no-op
    // (saturating_sub on a non-existent entry).
    tree.decref(id);
    assert!(tree.style(id).is_none());
    tree.decref(id); // no panic
}

fn decref_on_nonexistent_is_noop() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let id = el.as_node().tree_id().unwrap().1;
    tree.remove(id);
    // id is now stale.
    tree.decref(id); // no panic
    tree.incref(id); // no panic
}

/// Verifies the new_internal_leaf + transitive-reachability-GC
/// mechanism: an internal child (no Node) is automatically removed
/// when its parent is removed.
fn scroll_view_wrapper_is_cleaned_up_when_parent_drops() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let scroll = Element::create_scroll_view(&tree).0;
    let scroll_id = scroll.as_node().tree_id().unwrap().1;
    let wrapper_id = scroll
        .as_node()
        .with_meta(|m| m.child_taffy_parent)
        .expect("scroll_view allocates a wrapper");

    // Both exist.
    assert!(tree.style(scroll_id).is_some());
    assert!(tree.style(wrapper_id).is_some());
    assert_eq!(
        tree.refcount_for_test(wrapper_id),
        Some(0),
        "internal wrapper starts at refcount=0"
    );

    // Drop the scroll_view. The wrapper has refcount=0 and parent
    // becomes None; the transitive sweep in tree.remove cleans it up.
    drop(scroll);
    assert!(
        tree.style(scroll_id).is_none(),
        "scroll_view entry removed"
    );
    assert!(
        tree.style(wrapper_id).is_none(),
        "wrapper (refcount=0, orphaned) auto-removed by reachability GC"
    );
}

// =====================================================================
// 8. NSView identity stable across repeated accesses
// =====================================================================

fn ns_view_pointer_stable() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let ptr_before: *const objc2_app_kit::NSView = el.as_node().ns_view();
    let ptr_after: *const objc2_app_kit::NSView = el.as_node().ns_view();
    assert_eq!(ptr_before, ptr_after, "ns_view() pointer must be stable");
}

// =====================================================================
// 8. WeakNode / WeakElement — cycle-safe closure capture (Phase 4)
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

fn weak_element_upgrade_round_trips() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create_button(&tree).0;
    let weak = el.weak();
    let recovered = weak.upgrade().expect("alive");
    assert!(recovered.as_node().ptr_eq(el.as_node()));
}

/// THE regression guard for the Element-capture cycle.
///
/// If a handler closure captures `el.clone()` (strong), the cycle
/// keeps the arena entry alive forever. With `WeakElement` capture,
/// the entry should drop when the Element drops.
fn closure_capturing_weak_element_does_not_keep_arena_alive() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let tree = layout::new_tree();
    let id = {
        let el = Element::create_button(&tree).0;
        let id = el.as_node().tree_id().unwrap().1;

        // The dangerous-looking-but-actually-safe pattern: capture a
        // WeakElement in the click handler. If we'd captured `el.clone()`
        // instead, the cycle would keep the arena entry (and handler)
        // alive past the `drop(el)` below.
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
        // `el` drops here. WeakElement in the closure doesn't pin the
        // arena entry.
    };

    // After Element drop:
    // - Rc<NodeInner> hits 0 (the closure's WeakElement doesn't count)
    // - NodeInner::Drop → tree.decref(id) → arena removes the entry
    // - NodeData drop fires NodeHandlers::Drop → ActionTarget releases
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

/// Counter-example: the OLD pattern (capturing a strong Element)
/// keeps everything alive. This isn't asserting it as desired — it's
/// asserting the bug shape, so we can document the rule clearly and
/// catch any future "the cycle is closed" mistake.
fn strong_element_capture_keeps_handler_alive() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let tree = layout::new_tree();
    let id = {
        let el = Element::create_button(&tree).0;
        let id = el.as_node().tree_id().unwrap().1;

        // The DANGEROUS pattern: capturing a strong Element clone.
        let el_clone = el.clone();
        el.on_click(move || {
            let _ = el_clone.as_node().tree_id();  // touch it so the capture isn't optimized away
        });
        id
        // `el` drops here, but el_clone is still in the closure's
        // capture, which is held by the ActionTarget retained by
        // the arena entry's NodeHandlers. Cycle. Entry stays alive.
    };

    // Demonstrate the cycle: arena entry is STILL alive after the
    // user dropped their only Element handle, because the closure
    // holds a strong clone that the arena's handler ultimately owns.
    assert!(
        tree.style(id).is_some(),
        "documented cycle bug: strong Element capture keeps arena entry alive"
    );
    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "documented cycle bug: ActionTarget is leaked"
    );

    // Tear the tree down explicitly to clean up before the test
    // returns (otherwise the leak counter trips other tests).
    tree.remove(id);
    // After explicit removal the closure drops, el_clone drops, and
    // baseline should be restored.
    assert_eq!(handler_store_size_for_test(), baseline);
}

// =====================================================================
// Runner
// =====================================================================

fn main() {
    common::run_tests(&[
        ("freshly_created_node_has_tree_id", freshly_created_node_has_tree_id),
        ("style_mutation_lands_in_arena", style_mutation_lands_in_arena),
        ("scroll_view_has_child_taffy_parent_at_create_time", scroll_view_has_child_taffy_parent_at_create_time),
        ("dropping_last_node_clone_removes_arena_entry", dropping_last_node_clone_removes_arena_entry),
        ("cloning_node_extends_lifetime", cloning_node_extends_lifetime),
        ("borrowed_node_drop_does_not_remove_arena_entry", borrowed_node_drop_does_not_remove_arena_entry),
        ("handler_installed_drops_with_node", handler_installed_drops_with_node),
        ("text_field_delegate_releases_on_node_drop", text_field_delegate_releases_on_node_drop),
        ("new_leaf_starts_at_refcount_one", new_leaf_starts_at_refcount_one),
        ("incref_increments_refcount", incref_increments_refcount),
        ("decref_decrements_but_keeps_alive_if_attached", decref_decrements_but_keeps_alive_if_attached),
        ("detached_orphan_with_refcount_zero_is_removed", detached_orphan_with_refcount_zero_is_removed),
        ("detached_orphan_with_handles_stays", detached_orphan_with_handles_stays),
        ("decref_below_zero_is_safe", decref_below_zero_is_safe),
        ("decref_on_nonexistent_is_noop", decref_on_nonexistent_is_noop),
        ("scroll_view_wrapper_is_cleaned_up_when_parent_drops", scroll_view_wrapper_is_cleaned_up_when_parent_drops),
        ("ns_view_pointer_stable", ns_view_pointer_stable),
        ("weak_node_upgrades_while_node_alive", weak_node_upgrades_while_node_alive),
        ("weak_node_upgrade_fails_after_drop", weak_node_upgrade_fails_after_drop),
        ("weak_element_upgrade_round_trips", weak_element_upgrade_round_trips),
        ("closure_capturing_weak_element_does_not_keep_arena_alive", closure_capturing_weak_element_does_not_keep_arena_alive),
        ("strong_element_capture_keeps_handler_alive", strong_element_capture_keeps_handler_alive),
    ]);
}
