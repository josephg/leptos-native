//! Tests for the `Node` lifecycle under the thread-local store (iOS).
//!
//! A `Node` is a `Copy` `NodeId` into the per-thread store. There is
//! no refcount and no drop-driven removal: a node is created
//! Unattached, becomes Attached via `add_child`, and is Freed only by
//! an explicit `teardown` / `remove` (which cascades to the structural
//! subtree). Stale ids resolve to `None`/no-op via the generational
//! slotmap key.

#![cfg(target_os = "ios")]

mod common;

use ios_dom::{
    event::{handler_store_size_for_test, text_view_store_size_for_test},
    layout,
    Element,
};

fn freshly_created_node_is_in_store() {
    let el = Element::create_button().0;
    assert!(layout::style(el.as_node().id()).is_some());
}

fn style_mutation_lands_in_store() {
    let el = Element::create_vstack();
    el.as_node().with_style_mut(|s| s.flex_grow = 7.0);
    assert_eq!(layout::style(el.as_node().id()).unwrap().flex_grow, 7.0);
}

fn scroll_view_has_is_scroll_view_at_create_time() {
    let el = Element::create_scroll_view().0;
    assert!(el.as_node().with_meta(|m| m.is_scroll_view));
}

fn teardown_removes_store_entry() {
    let el = Element::create_button().0;
    let id = el.as_node().id();
    assert!(layout::style(id).is_some());
    el.as_node().teardown();
    assert!(layout::style(id).is_none());
}

fn copying_node_id_does_not_affect_lifetime() {
    let el = Element::create_button().0;
    let id = el.as_node().id();
    let copy = *el.as_node();
    let _ = copy;
    assert!(layout::style(id).is_some());
    el.as_node().teardown();
    assert!(layout::style(id).is_none());
}

fn handler_released_on_teardown() {
    let baseline = handler_store_size_for_test();
    let el = Element::create_button().0;
    el.on_click(|| {});
    assert_eq!(handler_store_size_for_test(), baseline + 1);
    el.as_node().teardown();
    assert_eq!(handler_store_size_for_test(), baseline);
}

fn text_view_delegate_releases_on_teardown() {
    let baseline = text_view_store_size_for_test();
    let el = Element::create_text_view().0;
    el.on_text_view_change(|_| {});
    assert_eq!(text_view_store_size_for_test(), baseline + 1);
    el.as_node().teardown();
    assert_eq!(text_view_store_size_for_test(), baseline);
}

fn teardown_cascades_to_children() {
    let root = Element::create_vstack();
    let child = Element::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());
    let root_id = root.as_node().id();
    let child_id = child.as_node().id();
    assert!(layout::style(root_id).is_some());
    assert!(layout::style(child_id).is_some());
    root.as_node().teardown();
    assert!(layout::style(root_id).is_none());
    assert!(layout::style(child_id).is_none());
}

fn detach_does_not_free() {
    let root = Element::create_vstack();
    let child = Element::create_button().0;
    layout::attach_child(root.as_node(), child.as_node());
    let child_id = child.as_node().id();
    layout::detach_child(root.as_node(), child.as_node());
    assert!(layout::style(child_id).is_some());
    assert_eq!(layout::parent(child_id), None);
    child.as_node().teardown();
    assert!(layout::style(child_id).is_none());
}

fn stale_id_accessors_are_safe() {
    let el = Element::create_button().0;
    let id = el.as_node().id();
    layout::remove(id);
    assert!(layout::style(id).is_none());
    assert!(layout::children(id).is_empty());
    assert_eq!(layout::parent(id), None);
    layout::remove(id);
}

fn ui_view_pointer_stable() {
    let el = Element::create_button().0;
    let p1: *const objc2_ui_kit::UIView = &*el.as_node().ui_view();
    let p2: *const objc2_ui_kit::UIView = &*el.as_node().ui_view();
    assert_eq!(p1, p2);
}

fn weak_node_upgrades_while_present() {
    let el = Element::create_button().0;
    let weak = el.as_node().downgrade();
    assert!(weak.is_alive());
    assert!(weak.upgrade().expect("alive").ptr_eq(el.as_node()));
}

fn weak_node_upgrade_fails_after_teardown() {
    let el = Element::create_button().0;
    let weak = el.as_node().downgrade();
    el.as_node().teardown();
    assert!(!weak.is_alive());
    assert!(weak.upgrade().is_none());
}

fn closure_capturing_node_does_not_pin_entry() {
    let baseline = handler_store_size_for_test();
    let el = Element::create_button().0;
    let id = el.as_node().id();
    let captured = *el.as_node();
    el.on_click(move || {
        let _ = captured.id();
    });
    assert_eq!(handler_store_size_for_test(), baseline + 1);
    el.as_node().teardown();
    assert!(layout::style(id).is_none());
    assert_eq!(handler_store_size_for_test(), baseline);
}

fn main() {
    common::run_tests(&[
        ("freshly_created_node_is_in_store", freshly_created_node_is_in_store),
        ("style_mutation_lands_in_store", style_mutation_lands_in_store),
        ("scroll_view_has_is_scroll_view_at_create_time", scroll_view_has_is_scroll_view_at_create_time),
        ("teardown_removes_store_entry", teardown_removes_store_entry),
        ("copying_node_id_does_not_affect_lifetime", copying_node_id_does_not_affect_lifetime),
        ("handler_released_on_teardown", handler_released_on_teardown),
        ("text_view_delegate_releases_on_teardown", text_view_delegate_releases_on_teardown),
        ("teardown_cascades_to_children", teardown_cascades_to_children),
        ("detach_does_not_free", detach_does_not_free),
        ("stale_id_accessors_are_safe", stale_id_accessors_are_safe),
        ("ui_view_pointer_stable", ui_view_pointer_stable),
        ("weak_node_upgrades_while_present", weak_node_upgrades_while_present),
        ("weak_node_upgrade_fails_after_teardown", weak_node_upgrade_fails_after_teardown),
        ("closure_capturing_node_does_not_pin_entry", closure_capturing_node_does_not_pin_entry),
    ]);
}
