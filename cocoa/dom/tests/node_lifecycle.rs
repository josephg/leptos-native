//! Tests for the new `Node` state machine (`Unmounted` / `Mounted` /
//! `MountedBorrowed`) and the lifecycle invariants that the
//! 2026-05-18 ownership refactor introduced.
//!
//! These complement `tree_mutation.rs`, `layout_dirtying.rs`, and
//! `events.rs` — those exercise the *behaviour* of mounted nodes;
//! this file exercises the *state transitions* themselves.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{
    event::{handler_store_size_for_test, text_field_store_size_for_test},
    layout::{self, register_in_tree},
    Element, Node, NodeKind,
};

// =====================================================================
// 1. tree_id / mounted_handle reflect state transitions
// =====================================================================

fn unmounted_node_has_no_tree_id() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    assert!(
        el.as_node().tree_id().is_none(),
        "fresh Node should be Unmounted"
    );
    assert!(el.as_node().mounted_handle().is_none());
}

fn register_transitions_to_mounted() {
    let _mtm = common::test_mtm();
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
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id1 = el.as_node().tree_id().unwrap().1;
    register_in_tree(el.as_node(), &tree);
    let id2 = el.as_node().tree_id().unwrap().1;
    assert_eq!(id1, id2, "second register should not allocate a new id");
}

// =====================================================================
// 2. Style/meta accessors route correctly per state
// =====================================================================

fn style_set_premount_survives_mount() {
    let _mtm = common::test_mtm();
    let el = Element::create("view");
    // Mutate before mount.
    el.as_node().with_style_mut(|s| s.flex_grow = 3.5);

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    // After mount, the value should be in the arena.
    let id = el.as_node().tree_id().unwrap().1;
    assert_eq!(
        tree.style(id).unwrap().flex_grow,
        3.5,
        "premount style mutation must migrate into the arena"
    );
    // And readable through `with_style`.
    el.as_node().with_style(|s| assert_eq!(s.flex_grow, 3.5));
}

fn style_set_postmount_lands_in_arena() {
    let _mtm = common::test_mtm();
    let el = Element::create("view");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id = el.as_node().tree_id().unwrap().1;

    // Mutate after mount.
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);

    assert_eq!(
        tree.style(id).unwrap().flex_grow,
        7.0,
        "post-mount style mutation must reach the arena"
    );
}

fn meta_set_premount_survives_mount() {
    let _mtm = common::test_mtm();
    // <scroll_view> sets meta.is_scroll_view=true in create_with;
    // verify the value is preserved through mount.
    let el = Element::create("scroll_view");
    el.as_node().with_style_mut(|s| s.flex_grow = 1.0);

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id = el.as_node().tree_id().unwrap().1;

    let meta = tree.meta(id).unwrap();
    assert!(
        meta.is_scroll_view,
        "is_scroll_view set premount must survive mount"
    );
    // The wrapper id should also be set by register_in_tree's scroll
    // view branch.
    assert!(
        meta.child_taffy_parent.is_some(),
        "scroll_view should have allocated a child_taffy_parent on mount"
    );
}

// =====================================================================
// 3. Drop ordering: arena entry goes away when last Node clone drops
// =====================================================================

fn dropping_last_node_clone_removes_arena_entry() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let id = {
        let el = Element::create("button");
        register_in_tree(el.as_node(), &tree);
        el.as_node().tree_id().unwrap().1
        // `el` drops here.
    };
    // After the only Node clone drops, the arena entry should be
    // gone — verified by tree.style returning None.
    assert!(
        tree.style(id).is_none(),
        "arena entry must be removed when last Node clone drops"
    );
}

fn cloning_node_extends_lifetime() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();
    let el = Element::create("button");
    register_in_tree(el.as_node(), &tree);
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
// 4. MountedBorrowed: from_view_with_handle Node does NOT remove
//    arena entries when dropped (so the "real" owner's entry stays).
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let _mtm = common::test_mtm();
    let mtm = common::test_mtm();
    let tree = layout::new_tree();

    let owner = Element::create_with("vstack", mtm);
    register_in_tree(owner.as_node(), &tree);
    let id = owner.as_node().tree_id().unwrap().1;

    // Synthesise a borrowed wrapper using the owner's handle.
    let handle = owner.as_node().mounted_handle().unwrap();
    let view_retained = owner.as_node().ns_view_retained();
    let borrowed = Node::from_view_with_handle(
        view_retained,
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
// 5. Handler-lifecycle: installing on Unmounted node and then
//    mounting moves the handler into the arena; dropping the Node
//    fires NodeHandlers::Drop (LiveTracker decrements).
// =====================================================================

fn handler_installed_premount_drops_with_node() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    {
        let el = Element::create("button");
        // Install on:click while still Unmounted.
        el.on_click(|| {});
        assert_eq!(
            handler_store_size_for_test(),
            baseline + 1,
            "on_click should allocate one ActionTarget"
        );

        // Mount, then drop the whole element. The arena entry
        // takes ownership of the handler bundle; dropping the
        // element triggers tree.remove which drops the handler.
        let tree = layout::new_tree();
        register_in_tree(el.as_node(), &tree);
    }
    // After everything drops, the action target should be gone.
    assert_eq!(
        handler_store_size_for_test(),
        baseline,
        "ActionTarget should dealloc when Node + tree go away"
    );
}

fn handler_installed_postmount_drops_with_tree_remove() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    {
        let el = Element::create("button");
        let tree = layout::new_tree();
        register_in_tree(el.as_node(), &tree);

        // Install after mount — routes to arena handlers.
        el.on_click(|| {});
        assert_eq!(handler_store_size_for_test(), baseline + 1);

        // Drop everything; same expected cleanup.
    }
    assert_eq!(handler_store_size_for_test(), baseline);
}

// =====================================================================
// 6. Text-field delegate cleanup: the field-drop-order fix in
//    NodeHandlers::Drop should ensure delegates dealloc cleanly.
// =====================================================================

fn text_field_delegate_releases_on_node_drop() {
    let _mtm = common::test_mtm();
    let baseline = text_field_store_size_for_test();
    {
        let el = Element::create("text_field");
        el.on_text_change(|_| {});
        assert_eq!(
            text_field_store_size_for_test(),
            baseline + 1,
            "ensure_text_field_entry should allocate one TextFieldDelegate"
        );
    }
    // Drop fires NodeHandlers::Drop with the explicit-drop fix; the
    // delegate's underlying ObjC object must dealloc.
    assert_eq!(
        text_field_store_size_for_test(),
        baseline,
        "TextFieldDelegate must dealloc when the Node drops — \
         regression test for the field-drop-order fix in \
         NodeHandlers::drop"
    );
}

// =====================================================================
// 7. with_handlers_mut routes correctly across mount transition
// =====================================================================

fn with_handlers_mut_works_in_both_states() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let el = Element::create("button");

    // Pre-mount: install a handler. Routes to local NodeHandlers.
    el.on_click(|| {});
    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "pre-mount install routes to local handlers and allocates ActionTarget"
    );

    // Mount; install migrates with the state into the arena.
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    // The previously-installed handler should still exist in the arena.
    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "ActionTarget should persist across the mount transition"
    );
}

// =====================================================================
// 8. NSView identity stable across mount transition
// =====================================================================

fn ns_view_pointer_stable_through_mount() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    let ptr_before: *const objc2_app_kit::NSView = el.as_node().ns_view();

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    let ptr_after: *const objc2_app_kit::NSView = el.as_node().ns_view();
    assert_eq!(
        ptr_before, ptr_after,
        "ns_view() must return the same underlying pointer before and after mount"
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
        ("meta_set_premount_survives_mount", meta_set_premount_survives_mount),
        ("dropping_last_node_clone_removes_arena_entry", dropping_last_node_clone_removes_arena_entry),
        ("cloning_node_extends_lifetime", cloning_node_extends_lifetime),
        ("borrowed_node_drop_does_not_remove_arena_entry", borrowed_node_drop_does_not_remove_arena_entry),
        ("handler_installed_premount_drops_with_node", handler_installed_premount_drops_with_node),
        ("handler_installed_postmount_drops_with_tree_remove", handler_installed_postmount_drops_with_tree_remove),
        ("text_field_delegate_releases_on_node_drop", text_field_delegate_releases_on_node_drop),
        ("with_handlers_mut_works_in_both_states", with_handlers_mut_works_in_both_states),
        ("ns_view_pointer_stable_through_mount", ns_view_pointer_stable_through_mount),
    ]);
}
