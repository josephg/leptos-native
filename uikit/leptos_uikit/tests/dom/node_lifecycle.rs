//! Tests for the `Node` lifecycle under the thread-local store (iOS).
//!
//! A `Node` is a `Copy` `NodeId` into the per-thread store. There is
//! no refcount and no drop-driven removal: a node is created
//! Unattached, becomes Attached via `add_child`, and is Freed only by
//! an explicit `teardown` / `remove` (which cascades to the structural
//! subtree). Stale ids resolve to `None`/no-op via the generational
//! slotmap key.


#[cfg(target_os = "ios")]
mod common;

#[cfg(target_os = "ios")]
mod ios {
    use super::common;

    use leptos_uikit::dom::{
        event::{handler_store_size_for_test, text_view_store_size_for_test},
        layout,
        UikitElem, UikitMakeView, UikitNodeExt,
    };

    fn freshly_created_node_is_in_store() {
        let el = UikitElem::create_button().0;
        assert!(layout::style(el.id()).is_some());
    }

    fn style_mutation_lands_in_store() {
        let el = UikitElem::create_vstack();
        el.with_style_mut(|s| s.flex_grow = 7.0);
        assert_eq!(layout::style(el.id()).unwrap().flex_grow, 7.0);
    }

    fn scroll_view_has_is_scroll_view_at_create_time() {
        let el = UikitElem::create_scroll_view().0;
        assert!(el.with_meta(|m| m.is_scroll_view));
    }

    fn teardown_removes_store_entry() {
        let el = UikitElem::create_button().0;
        let id = el.id();
        assert!(layout::style(id).is_some());
        el.teardown();
        assert!(layout::style(id).is_none());
    }

    fn copying_node_id_does_not_affect_lifetime() {
        let el = UikitElem::create_button().0;
        let id = el.id();
        let copy = el;
        let _ = copy;
        assert!(layout::style(id).is_some());
        el.teardown();
        assert!(layout::style(id).is_none());
    }

    fn handler_released_on_teardown() {
        let baseline = handler_store_size_for_test();
        let el = UikitElem::create_button().0;
        el.on_click(|| {});
        assert_eq!(handler_store_size_for_test(), baseline + 1);
        el.teardown();
        assert_eq!(handler_store_size_for_test(), baseline);
    }

    fn text_view_delegate_releases_on_teardown() {
        let baseline = text_view_store_size_for_test();
        let el = UikitElem::create_text_view().0;
        el.on_text_view_change(|_| {});
        assert_eq!(text_view_store_size_for_test(), baseline + 1);
        el.teardown();
        assert_eq!(text_view_store_size_for_test(), baseline);
    }

    fn teardown_cascades_to_children() {
        let root = UikitElem::create_vstack();
        let child = UikitElem::create_button().0;
        layout::attach_child(root, child);
        let root_id = root.id();
        let child_id = child.id();
        assert!(layout::style(root_id).is_some());
        assert!(layout::style(child_id).is_some());
        root.teardown();
        assert!(layout::style(root_id).is_none());
        assert!(layout::style(child_id).is_none());
    }

    fn detach_does_not_free() {
        let root = UikitElem::create_vstack();
        let child = UikitElem::create_button().0;
        layout::attach_child(root, child);
        let child_id = child.id();
        layout::detach_child(root, child);
        assert!(layout::style(child_id).is_some());
        assert_eq!(layout::parent(child_id), None);
        child.teardown();
        assert!(layout::style(child_id).is_none());
    }

    fn stale_id_accessors_are_safe() {
        let el = UikitElem::create_button().0;
        let id = el.id();
        layout::remove(id);
        assert!(layout::style(id).is_none());
        assert!(layout::children(id).is_empty());
        assert_eq!(layout::parent(id), None);
        layout::remove(id);
    }

    fn ui_view_pointer_stable() {
        let el = UikitElem::create_button().0;
        let p1: *const objc2_ui_kit::UIView = &*el.ui_view();
        let p2: *const objc2_ui_kit::UIView = &*el.ui_view();
        assert_eq!(p1, p2);
    }

    fn closure_capturing_node_does_not_pin_entry() {
        let baseline = handler_store_size_for_test();
        let el = UikitElem::create_button().0;
        let id = el.id();
        let captured = el;
        el.on_click(move || {
            let _ = captured.id();
        });
        assert_eq!(handler_store_size_for_test(), baseline + 1);
        el.teardown();
        assert!(layout::style(id).is_none());
        assert_eq!(handler_store_size_for_test(), baseline);
    }

    // A mounted subtree returns the store to baseline after the root is
    // torn down — the headless analogue of whole-scene teardown
    // (UIApplicationMain owns the loop, so we can't open a real scene in
    // a unit test). Locks in the explicit-free lifecycle the
    // `ElementState::Drop` safety net relies on.
    fn subtree_teardown_returns_to_baseline() {
        let baseline = layout::node_count();

        let root = UikitElem::create_vstack();
        let row = UikitElem::create_hstack();
        let b1 = UikitElem::create_button().0;
        let b2 = UikitElem::create_button().0;
        let label = UikitElem::create_label().0;
        layout::attach_child(row, b1);
        layout::attach_child(row, b2);
        layout::attach_child(root, label);
        layout::attach_child(root, row);

        assert!(layout::node_count() > baseline, "mounting grows the store");

        root.teardown();
        assert_eq!(
            layout::node_count(),
            baseline,
            "store returned to baseline after subtree teardown — no leak"
        );
    }

    fn unattached_node_teardown_returns_to_baseline() {
        let baseline = layout::node_count();
        let el = UikitElem::create_button().0;
        assert_eq!(layout::node_count(), baseline + 1);
        el.teardown();
        assert_eq!(
            layout::node_count(),
            baseline,
            "unattached node freed by teardown — no orphan leak"
        );
    }

    pub fn run() {
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
            ("closure_capturing_node_does_not_pin_entry", closure_capturing_node_does_not_pin_entry),
            ("subtree_teardown_returns_to_baseline", subtree_teardown_returns_to_baseline),
            ("unattached_node_teardown_returns_to_baseline", unattached_node_teardown_returns_to_baseline),
        ]);
    }
}

#[cfg(target_os = "ios")]
fn main() {
    ios::run();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("ios tests not run on non-ios platform");
}