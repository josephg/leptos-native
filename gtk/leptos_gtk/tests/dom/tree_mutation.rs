//! Tree-mutation tests for `Element`: `insert_node`, `remove_child`,
//! `clear_children`, and node identity (`ptr_eq`, into_node round-trips).

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::dom::GtkNode;
use leptos_gtk::gtk4::prelude::*;

// ---------------------------------------------------------------------
// Identity / round-trip
// ---------------------------------------------------------------------

fn ptr_eq_false_for_distinct() {
    let a = GtkNode::create_stack();
    let b = GtkNode::create_stack();
    assert_ne!(
        a, b,
        "distinct Elements should not be equal"
    );
}

// ---------------------------------------------------------------------
// Helpers — count children of a gtk::Box / Widget
// ---------------------------------------------------------------------

fn child_count(widget: &gtk4::Widget) -> usize {
    let mut cur = widget.first_child();
    let mut n = 0usize;
    while let Some(w) = cur {
        n += 1;
        cur = w.next_sibling();
    }
    n
}

fn child_at(widget: &gtk4::Widget, idx: usize) -> Option<gtk4::Widget> {
    let mut cur = widget.first_child();
    let mut i = 0usize;
    while let Some(w) = cur {
        if i == idx {
            return Some(w);
        }
        i += 1;
        cur = w.next_sibling();
    }
    None
}

// ---------------------------------------------------------------------
// insert_node
// ---------------------------------------------------------------------

fn insert_node_appends_when_marker_none() {
    let parent = GtkNode::create_stack();
    let a = GtkNode::create_button().0;
    let b = GtkNode::create_button().0;

    parent.insert_node(a, None);
    parent.insert_node(b, None);

    assert_eq!(child_count(&parent.widget()), 2);
    let first = child_at(&parent.widget(), 0).unwrap();
    let second = child_at(&parent.widget(), 1).unwrap();
    assert_eq!(first.as_ptr(), a.widget().as_ptr());
    assert_eq!(second.as_ptr(), b.widget().as_ptr());
}

fn insert_node_before_marker_places_correctly() {
    let parent = GtkNode::create_stack();
    let a = GtkNode::create_button().0;
    let b = GtkNode::create_button().0;
    let c = GtkNode::create_button().0;

    // Initial order: a, c
    parent.insert_node(a, None);
    parent.insert_node(c, None);
    // Insert b before c
    parent.insert_node(b, Some(c));

    assert_eq!(child_count(&parent.widget()), 3);
    assert_eq!(
        child_at(&parent.widget(), 0).unwrap().as_ptr(),
        a.widget().as_ptr()
    );
    assert_eq!(
        child_at(&parent.widget(), 1).unwrap().as_ptr(),
        b.widget().as_ptr()
    );
    assert_eq!(
        child_at(&parent.widget(), 2).unwrap().as_ptr(),
        c.widget().as_ptr()
    );
}

fn insert_node_moves_existing_child() {
    // gtk::Widget semantics: a widget has one parent. Inserting it
    // under a new parent removes it from the old.
    let parent_a = GtkNode::create_stack();
    let parent_b = GtkNode::create_stack();
    let child = GtkNode::create_button().0;

    parent_a.insert_node(child, None);
    assert_eq!(child_count(&parent_a.widget()), 1);

    parent_b.insert_node(child, None);
    assert_eq!(child_count(&parent_a.widget()), 0);
    assert_eq!(child_count(&parent_b.widget()), 1);
}

// ---------------------------------------------------------------------
// remove_child
// ---------------------------------------------------------------------

fn remove_child_returns_some_for_actual_child() {
    let parent = GtkNode::create_stack();
    let child = GtkNode::create_button().0;
    parent.insert_node(child, None);

    let removed = parent.remove_child(child);
    assert!(removed.is_some());
    assert_eq!(child_count(&parent.widget()), 0);
}

fn remove_child_returns_none_for_non_child() {
    let parent = GtkNode::create_stack();
    let actual = GtkNode::create_button().0;
    let stranger = GtkNode::create_button().0;
    parent.insert_node(actual, None);

    let removed = parent.remove_child(stranger);
    assert!(removed.is_none(), "non-child remove returns None");

    assert_eq!(child_count(&parent.widget()), 1);
}

// ---------------------------------------------------------------------
// clear_children
// ---------------------------------------------------------------------

fn clear_children_removes_all() {
    let parent = GtkNode::create_stack();
    for _ in 0..5 {
        parent.insert_node(GtkNode::create_button().0, None);
    }
    assert_eq!(child_count(&parent.widget()), 5);

    parent.clear_children();
    assert_eq!(child_count(&parent.widget()), 0);
}

fn clear_children_on_empty_is_no_op() {
    let parent = GtkNode::create_stack();
    parent.clear_children();
    parent.clear_children();
    assert_eq!(child_count(&parent.widget()), 0);
}

fn main() {
    common::run_tests(&[
        // Identity / round-trip
        ("ptr_eq_false_for_distinct", ptr_eq_false_for_distinct),
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
