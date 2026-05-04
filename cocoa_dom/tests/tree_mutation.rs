//! Tree-mutation tests for `Element`: `insert_node`, `remove_child`,
//! `clear_children`, and node identity (`ptr_eq`, into_node round-trips).

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{Element, NodeKind, Text};

// ---------------------------------------------------------------------
// Identity / round-trip
// ---------------------------------------------------------------------

fn ptr_eq_true_for_clones() {
    let _mtm = common::test_mtm();
    let el = Element::create("view");
    let a = el.as_node().clone();
    let b = el.as_node().clone();
    assert!(a.ptr_eq(&b), "clones should pointer-eq");
}

fn ptr_eq_false_for_distinct() {
    let _mtm = common::test_mtm();
    let a = Element::create("view");
    let b = Element::create("view");
    assert!(
        !a.as_node().ptr_eq(b.as_node()),
        "distinct Elements should not pointer-eq"
    );
}

fn into_node_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    let original_ptr: *const objc2_app_kit::NSView = el.ns_view();
    let n = el.into_node();
    let el2 = Element::from_node_unchecked(n);
    let after_ptr: *const objc2_app_kit::NSView = el2.ns_view();
    assert_eq!(
        original_ptr, after_ptr,
        "into_node + from_node_unchecked should preserve NSView identity"
    );
    assert_eq!(el2.as_node().kind(), NodeKind::Element);
}

#[allow(unreachable_code)]
fn from_node_unchecked_panics_on_wrong_kind() {
    let _mtm = common::test_mtm();
    let t = Text::create("x");
    let n = t.into_node();
    // Passing a Text node into Element::from_node_unchecked should
    // panic — the kind enum is checked.
    let result = std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(move || {
            let _ = Element::from_node_unchecked(n);
        }),
    );
    assert!(
        result.is_err(),
        "from_node_unchecked should panic on a non-Element node"
    );
}

// ---------------------------------------------------------------------
// insert_node
// ---------------------------------------------------------------------

fn insert_node_appends_when_marker_none() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    let a = Element::create("button");
    let b = Element::create("button");

    parent.insert_node(a.as_node(), None);
    parent.insert_node(b.as_node(), None);

    let subs: Vec<_> = parent.ns_view().subviews().iter().collect();
    assert_eq!(subs.len(), 2);
    assert_eq!(&*subs[0] as *const _, a.ns_view() as *const _);
    assert_eq!(&*subs[1] as *const _, b.ns_view() as *const _);
}

fn insert_node_before_marker_places_correctly() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    let a = Element::create("button");
    let b = Element::create("button");
    let c = Element::create("button");

    // Initial order: a, c
    parent.insert_node(a.as_node(), None);
    parent.insert_node(c.as_node(), None);
    // Insert b before c
    parent.insert_node(b.as_node(), Some(c.as_node()));

    let subs: Vec<_> = parent.ns_view().subviews().iter().collect();
    assert_eq!(subs.len(), 3);
    assert_eq!(&*subs[0] as *const _, a.ns_view() as *const _);
    assert_eq!(&*subs[1] as *const _, b.ns_view() as *const _);
    assert_eq!(&*subs[2] as *const _, c.ns_view() as *const _);
}

// (insert_node behavior with an "unrelated marker" — i.e. one that
// isn't actually a child of the parent — is implementation-
// defined and not documented. Test omitted; cover when behavior
// is pinned down.)

fn insert_node_moves_existing_child() {
    // NSView semantics: a view has one parent. Inserting it under a
    // new parent removes it from the old.
    let _mtm = common::test_mtm();
    let parent_a = Element::create("view");
    let parent_b = Element::create("view");
    let child = Element::create("button");

    parent_a.insert_node(child.as_node(), None);
    assert_eq!(parent_a.ns_view().subviews().len(), 1);

    parent_b.insert_node(child.as_node(), None);
    // child should have moved
    assert_eq!(parent_a.ns_view().subviews().len(), 0);
    assert_eq!(parent_b.ns_view().subviews().len(), 1);
}

// ---------------------------------------------------------------------
// remove_child
// ---------------------------------------------------------------------

fn remove_child_returns_some_for_actual_child() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    let child = Element::create("button");
    parent.insert_node(child.as_node(), None);

    let removed = parent.remove_child(child.as_node());
    assert!(removed.is_some());
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn remove_child_returns_none_for_non_child() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    let actual = Element::create("button");
    let stranger = Element::create("button");
    parent.insert_node(actual.as_node(), None);

    let removed = parent.remove_child(stranger.as_node());
    assert!(removed.is_none(), "non-child remove returns None");

    // The actual child stays put.
    assert_eq!(parent.ns_view().subviews().len(), 1);
}

// ---------------------------------------------------------------------
// clear_children
// ---------------------------------------------------------------------

fn clear_children_removes_all() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    for _ in 0..5 {
        parent.insert_node(
            Element::create("button").as_node(),
            None,
        );
    }
    assert_eq!(parent.ns_view().subviews().len(), 5);

    parent.clear_children();
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn clear_children_on_empty_is_no_op() {
    let _mtm = common::test_mtm();
    let parent = Element::create("view");
    parent.clear_children();
    parent.clear_children();
    assert_eq!(parent.ns_view().subviews().len(), 0);
}

fn main() {
    common::run_tests(&[
        // Identity / round-trip
        ("ptr_eq_true_for_clones", ptr_eq_true_for_clones),
        ("ptr_eq_false_for_distinct", ptr_eq_false_for_distinct),
        ("into_node_round_trip", into_node_round_trip),
        (
            "from_node_unchecked_panics_on_wrong_kind",
            from_node_unchecked_panics_on_wrong_kind,
        ),
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
