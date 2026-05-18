//! Tests for the new `Node` state machine on the iOS port
//! (`Unmounted` / `Mounted` / `MountedBorrowed`) — mirrors
//! `cocoa/dom/tests/node_lifecycle.rs`.
//!
//! iOS handlers are similar to cocoa's: UIControl target/action +
//! UITextView delegate. The same explicit-delegate-drop fix in
//! `IosNodeHandlers::Drop` applies; the `text_view_delegate_…` test
//! is the regression guard.

#![cfg(target_os = "ios")]

mod common;

use ios_dom::{
    event::{handler_store_size_for_test, text_view_store_size_for_test},
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
    let _mtm = common::test_mtm();
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

fn meta_set_premount_survives_mount() {
    let _mtm = common::test_mtm();
    let el = Element::create("scroll_view");
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);
    let id = el.as_node().tree_id().unwrap().1;
    let meta = tree.meta(id).unwrap();
    assert!(
        meta.is_scroll_view,
        "is_scroll_view set premount must survive mount"
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
    };
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
// 4. MountedBorrowed: from_view_with_handle does NOT remove arena
//    entries when dropped.
// =====================================================================

fn borrowed_node_drop_does_not_remove_arena_entry() {
    let _mtm = common::test_mtm();
    let tree = layout::new_tree();

    let owner = Element::create("vstack");
    register_in_tree(owner.as_node(), &tree);
    let id = owner.as_node().tree_id().unwrap().1;

    let handle = owner.as_node().mounted_handle().unwrap();
    let view_retained = owner.as_node().ui_view_retained();
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
// 5. Handler-lifecycle: action-target install + drop balance
// =====================================================================

fn handler_installed_premount_drops_with_node() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    {
        let el = Element::create("button");
        el.on_click(|| {});
        assert_eq!(
            handler_store_size_for_test(),
            baseline + 1,
            "on_click should allocate one ActionTarget"
        );

        let tree = layout::new_tree();
        register_in_tree(el.as_node(), &tree);
    }
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

        el.on_click(|| {});
        assert_eq!(handler_store_size_for_test(), baseline + 1);
    }
    assert_eq!(handler_store_size_for_test(), baseline);
}

// =====================================================================
// 6. Text-view delegate cleanup: the explicit-drop fix in
//    IosNodeHandlers::Drop should ensure delegates dealloc cleanly.
// =====================================================================

fn text_view_delegate_releases_on_node_drop() {
    let _mtm = common::test_mtm();
    let baseline = text_view_store_size_for_test();
    {
        let el = Element::create("text_view");
        // iOS: text_view uses UITextViewDelegate (UITextView is not a
        // UIControl). Use the text-view-specific install, not the
        // text-field one.
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
// 7. with_handlers_mut routes correctly across mount transition
// =====================================================================

fn with_handlers_mut_works_in_both_states() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let el = Element::create("button");

    // Pre-mount: install routes to local NodeHandlers.
    el.on_click(|| {});
    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "pre-mount install allocates ActionTarget"
    );

    // Mount; install migrates with the state into the arena.
    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "ActionTarget should persist across the mount transition"
    );
}

// =====================================================================
// 8. UIView identity stable across mount transition
// =====================================================================

fn ui_view_pointer_stable_through_mount() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    let ptr_before: *const objc2_ui_kit::UIView = el.as_node().ui_view();

    let tree = layout::new_tree();
    register_in_tree(el.as_node(), &tree);

    let ptr_after: *const objc2_ui_kit::UIView = el.as_node().ui_view();
    assert_eq!(
        ptr_before, ptr_after,
        "ui_view() must return the same underlying pointer before and after mount"
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
        ("text_view_delegate_releases_on_node_drop", text_view_delegate_releases_on_node_drop),
        ("with_handlers_mut_works_in_both_states", with_handlers_mut_works_in_both_states),
        ("ui_view_pointer_stable_through_mount", ui_view_pointer_stable_through_mount),
    ]);
}
