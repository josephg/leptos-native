//! Tree-mutation tests for `Element`: `insert_node`, `remove_child`,
//! `clear_children`, and node identity (`ptr_eq`, into_node round-trips).

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::CocoaNode;

// ---------------------------------------------------------------------
// Identity / round-trip
// ---------------------------------------------------------------------

fn ptr_eq_false_for_distinct() {
    let _mtm = common::test_mtm();
    let a = CocoaNode::create_container();
    let b = CocoaNode::create_container();
    assert_ne!(
        a, b,
        "distinct Elements should not pointer-eq"
    );
}

fn into_node_round_trip() {
    let _mtm = common::test_mtm();
    let el = CocoaNode::create_button().0;
    let original_ptr: *const objc2_app_kit::NSView = &*el.ns_view();
    let n = el;
    let el2 = n;
    let after_ptr: *const objc2_app_kit::NSView = &*el2.ns_view();
    assert_eq!(
        original_ptr, after_ptr,
        "into_node should preserve NSView identity (Node = Element)"
    );
}

// ---------------------------------------------------------------------
// insert_node
// ---------------------------------------------------------------------

fn insert_node_appends_when_marker_none() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    let a = CocoaNode::create_button().0;
    let b = CocoaNode::create_button().0;

    parent.insert_node(a, None);
    parent.insert_node(b, None);

    let subs: Vec<_> = parent.ns_view().subviews().iter().collect();
    assert_eq!(subs.len(), 2);
    assert_eq!(&*subs[0] as *const _, &*a.ns_view() as *const _);
    assert_eq!(&*subs[1] as *const _, &*b.ns_view() as *const _);
}

fn insert_node_before_marker_places_correctly() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    let a = CocoaNode::create_button().0;
    let b = CocoaNode::create_button().0;
    let c = CocoaNode::create_button().0;

    // Initial order: a, c
    parent.insert_node(a, None);
    parent.insert_node(c, None);
    // Insert b before c
    parent.insert_node(b, Some(c));

    let subs: Vec<_> = parent.ns_view().subviews().iter().collect();
    assert_eq!(subs.len(), 3);
    assert_eq!(&*subs[0] as *const _, &*a.ns_view() as *const _);
    assert_eq!(&*subs[1] as *const _, &*b.ns_view() as *const _);
    assert_eq!(&*subs[2] as *const _, &*c.ns_view() as *const _);
}

// (insert_node behavior with an "unrelated marker" — i.e. one that
// isn't actually a child of the parent — is implementation-
// defined and not documented. Test omitted; cover when behavior
// is pinned down.)

fn insert_node_moves_existing_child() {
    // NSView semantics: a view has one parent. Inserting it under a
    // new parent removes it from the old.
    let _mtm = common::test_mtm();
    let parent_a = CocoaNode::create_container();
    let parent_b = CocoaNode::create_container();
    let child = CocoaNode::create_button().0;

    parent_a.insert_node(child, None);
    assert_eq!(parent_a.ns_view().subviews().len(), 1);

    parent_b.insert_node(child, None);
    // child should have moved
    assert_eq!(parent_a.ns_view().subviews().len(), 0);
    assert_eq!(parent_b.ns_view().subviews().len(), 1);
}

// ---------------------------------------------------------------------
// remove_child
// ---------------------------------------------------------------------

fn remove_child_returns_some_for_actual_child() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    let child = CocoaNode::create_button().0;
    parent.insert_node(child, None);

    let removed = parent.remove_child(child);
    assert!(removed.is_some());
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn remove_child_returns_none_for_non_child() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    let actual = CocoaNode::create_button().0;
    let stranger = CocoaNode::create_button().0;
    parent.insert_node(actual, None);

    let removed = parent.remove_child(stranger);
    assert!(removed.is_none(), "non-child remove returns None");

    // The actual child stays put.
    assert_eq!(parent.ns_view().subviews().len(), 1);
}

// ---------------------------------------------------------------------
// clear_children
// ---------------------------------------------------------------------

fn clear_children_removes_all() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    for _ in 0..5 {
        parent.insert_node(
            CocoaNode::create_button().0,
            None,
        );
    }
    assert_eq!(parent.ns_view().subviews().len(), 5);

    parent.clear_children();
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn clear_children_on_empty_is_no_op() {
    let _mtm = common::test_mtm();
    let parent = CocoaNode::create_container();
    parent.clear_children();
    parent.clear_children();
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn main() {
    common::run_tests(&[
        // Identity / round-trip
        ("ptr_eq_false_for_distinct", ptr_eq_false_for_distinct),
        ("into_node_round_trip", into_node_round_trip),
        // insert_node
        (
            "insert_node_appends_when_marker_none",
            insert_node_appends_when_marker_none,
        ),
        (
            "insert_node_before_marker_places_correctly",
            insert_node_before_marker_places_correctly,
        ),
        ("insert_node_moves_existing_child", insert_node_moves_existing_child),
        // remove_child
        (
            "remove_child_returns_some_for_actual_child",
            remove_child_returns_some_for_actual_child,
        ),
        (
            "remove_child_returns_none_for_non_child",
            remove_child_returns_none_for_non_child,
        ),
        // clear_children
        ("clear_children_removes_all", clear_children_removes_all),
        ("clear_children_on_empty_is_no_op", clear_children_on_empty_is_no_op),
    ]);
}
