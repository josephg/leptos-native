//! Tests for the `Node` lifecycle under the thread-local store.
//!
//! A `Node` is a `Copy` `NodeId` into the per-thread store. There is
//! no refcount and no drop-driven removal: a node is created
//! Unattached, becomes Attached via `add_child`, and is Freed only by
//! an explicit `teardown` / `remove` (which cascades to the structural
//! subtree). Stale ids resolve to `None`/no-op via the generational
//! slotmap key.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::{event::{handler_store_size_for_test, text_field_store_size_for_test}, layout, window, CocoaElem, CocoaMakeView, CocoaNodeExt};

// =====================================================================
// 1. Fresh nodes are in the store from creation
// =====================================================================

fn freshly_created_node_is_in_store() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    let id = el.id();
    assert!(
        layout::style(id).is_some(),
        "store entry exists for a freshly-created node"
    );
}

// =====================================================================
// 2. Style/meta accessors route through the store
// =====================================================================

fn style_mutation_lands_in_store() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_container();
    el.with_style_mut(|s| s.flex_grow = 7.0);
    let id = el.id();
    assert_eq!(layout::style(id).unwrap().flex_grow, 7.0);
}

fn scroll_view_has_child_taffy_parent_at_create_time() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_scroll_view().0;
    let meta = el.with_meta(|m| m.clone());
    assert!(meta.is_scroll_view, "scroll_view sets is_scroll_view");
    assert!(
        meta.child_taffy_parent.is_some(),
        "scroll_view allocates documentView wrapper eagerly"
    );
}

// =====================================================================
// 3. Lifecycle: explicit teardown removes the entry
// =====================================================================

fn teardown_removes_store_entry() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    let id = el.id();
    assert!(layout::style(id).is_some());

    el.teardown();
    assert!(
        layout::style(id).is_none(),
        "teardown removes the store entry"
    );
}

// =====================================================================
// 4. Handler lifecycle: teardown fires NodeHandlers::Drop
// =====================================================================

fn handler_released_on_teardown() {
    let _mtm = common::test_mtm();
    let baseline = handler_store_size_for_test();
    let el = CocoaElem::create_button().0;
    el.on_click(|| {});
    assert_eq!(
        handler_store_size_for_test(),
        baseline + 1,
        "on_click should allocate one ActionTarget"
    );
    el.teardown();
    assert_eq!(
        handler_store_size_for_test(),
        baseline,
        "ActionTarget should dealloc when the node is torn down"
    );
}

fn text_field_delegate_releases_on_teardown() {
    let _mtm = common::test_mtm();
    let baseline = text_field_store_size_for_test();
    let el = CocoaElem::create_text_field().0;
    el.on_text_change(|_| {});
    assert_eq!(
        text_field_store_size_for_test(),
        baseline + 1,
        "ensure_text_field_entry should allocate one TextFieldDelegate"
    );
    el.teardown();
    assert_eq!(
        text_field_store_size_for_test(),
        baseline,
        "TextFieldDelegate must dealloc on teardown — regression test \
         for the field-drop-order fix in NodeHandlers::drop"
    );
}

// =====================================================================
// 5. teardown cascades to the structural subtree
// =====================================================================

fn teardown_cascades_to_children() {
    let _mtm = common::test_mtm();
    let root = CocoaElem::create_container();
    let child = CocoaElem::create_button().0;
    layout::attach_child(root, child);

    let root_id = root.id();
    let child_id = child.id();
    assert!(layout::style(root_id).is_some());
    assert!(layout::style(child_id).is_some());

    root.teardown();
    assert!(
        layout::style(root_id).is_none(),
        "root removed by teardown"
    );
    assert!(
        layout::style(child_id).is_none(),
        "structural child removed by the teardown cascade"
    );
}

/// The scroll-view documentView wrapper is an internal structural
/// child (no Node owner). Tearing down the scroll_view must free it.
fn scroll_view_wrapper_freed_with_parent() {
    let _mtm = common::test_mtm();
    let scroll = CocoaElem::create_scroll_view().0;
    let scroll_id = scroll.id();
    let wrapper_id = scroll

        .with_meta(|m| m.child_taffy_parent)
        .expect("scroll_view allocates a wrapper");

    assert!(layout::style(scroll_id).is_some());
    assert!(layout::style(wrapper_id).is_some());

    scroll.teardown();
    assert!(
        layout::style(scroll_id).is_none(),
        "scroll_view entry removed"
    );
    assert!(
        layout::style(wrapper_id).is_none(),
        "internal wrapper freed by the teardown cascade"
    );
}

/// Detaching a child does NOT free it (it becomes Unattached but
/// stays in the store until explicitly removed).
fn detach_does_not_free() {
    let _mtm = common::test_mtm();
    let root = CocoaElem::create_container();
    let child = CocoaElem::create_button().0;
    layout::attach_child(root, child);
    let child_id = child.id();

    layout::detach_child(root, child);
    assert!(
        layout::style(child_id).is_some(),
        "detach leaves the node Unattached but present"
    );
    assert_eq!(
        layout::parent(child_id),
        None,
        "detached node has no parent"
    );

    child.teardown();
    assert!(layout::style(child_id).is_none());
}

// =====================================================================
// 6. Stale ids are safe no-ops
// =====================================================================

fn stale_id_accessors_are_safe() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    let id = el.id();
    layout::remove(id);
    // id is now stale.
    assert!(layout::style(id).is_none());
    assert!(layout::children(id).is_empty());
    assert_eq!(layout::parent(id), None);
    layout::remove(id); // double-remove is a no-op
}

// =====================================================================
// 7. NSView identity stable across repeated accesses
// =====================================================================

fn ns_view_pointer_stable() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    let ptr_before: *const objc2_app_kit::NSView = &*el.ns_view();
    let ptr_after: *const objc2_app_kit::NSView = &*el.ns_view();
    assert_eq!(ptr_before, ptr_after, "ns_view() pointer must be stable");
}

// =====================================================================
// 9. Whole-window teardown returns the store to its baseline
// =====================================================================

/// Open a window, mount a small subtree under its content_root, lay it
/// out, then tear the content_root down (what `WindowState`'s cleanup
/// closure does on window close). The shared node store must return to
/// the exact count it had before the window opened — no per-window
/// leak. This is the deterministic counterpart to the cocoa_fuzzer's
/// baseline check.
fn window_teardown_returns_to_baseline() {
    use objc2_foundation::NSSize;

    let mtm = common::test_mtm();
    let baseline = layout::node_count();

    let opened = window::open_window("leak-test", (320.0, 240.0), mtm);

    // A container with a label and two buttons under content_root.
    let row = CocoaElem::create_container();
    let b1 = CocoaElem::create_button().0;
    let b2 = CocoaElem::create_button().0;
    layout::attach_child(row, b1);
    layout::attach_child(row, b2);
    let label = CocoaElem::create_label().0;

    opened.content_root.insert_node(label, None);
    opened.content_root.insert_node(row, None);

    layout::compute_layout(opened.content_root, NSSize::new(320.0, 240.0));

    assert!(
        layout::node_count() > baseline,
        "mounting a subtree grows the store"
    );

    // Window close path: teardown the content_root (cascades the whole
    // subtree out of the store), then close the NSWindow.
    opened.content_root.teardown();
    opened.close();

    assert_eq!(
        layout::node_count(),
        baseline,
        "store returned to baseline after window teardown — no leak"
    );
}

/// An `ElementState`-style flow where the node is built but never
/// mounted, then dropped. Here we model it at the dom layer: a node
/// created and then dropped via `teardown` with no parent. (The
/// leptos-layer `ElementState::Drop` safety net relies on this same
/// idempotent teardown.) Confirms an unattached node is fully removed.
fn unattached_node_teardown_returns_to_baseline() {
    let _mtm = common::test_mtm();
    let baseline = layout::node_count();

    let el = CocoaElem::create_button().0;
    assert_eq!(layout::node_count(), baseline + 1);

    // Never attached to any parent — the orphan case. Explicit
    // teardown frees it.
    el.teardown();
    assert_eq!(
        layout::node_count(),
        baseline,
        "unattached node freed by teardown — no orphan leak"
    );
}

// =====================================================================
// Runner
// =====================================================================

fn main() {
    common::run_tests(&[
        ("freshly_created_node_is_in_store", freshly_created_node_is_in_store),
        ("style_mutation_lands_in_store", style_mutation_lands_in_store),
        ("scroll_view_has_child_taffy_parent_at_create_time", scroll_view_has_child_taffy_parent_at_create_time),
        ("teardown_removes_store_entry", teardown_removes_store_entry),
        ("handler_released_on_teardown", handler_released_on_teardown),
        ("text_field_delegate_releases_on_teardown", text_field_delegate_releases_on_teardown),
        ("teardown_cascades_to_children", teardown_cascades_to_children),
        ("scroll_view_wrapper_freed_with_parent", scroll_view_wrapper_freed_with_parent),
        ("detach_does_not_free", detach_does_not_free),
        ("stale_id_accessors_are_safe", stale_id_accessors_are_safe),
        ("ns_view_pointer_stable", ns_view_pointer_stable),
        ("window_teardown_returns_to_baseline", window_teardown_returns_to_baseline),
        ("unattached_node_teardown_returns_to_baseline", unattached_node_teardown_returns_to_baseline),
    ]);
}
