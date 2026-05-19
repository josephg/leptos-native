//! Tests for the text-label (`Element::create_text`) and placeholder
//! (`Element::create_placeholder`) constructors on `Element`.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{gtk::prelude::*, Element};

fn text_create_basic() {
    let tree = gtk_dom::layout::new_tree();
    let t = Element::create_text(&tree, "hello");

    let l = t
        .as_node()
        .widget()
        .downcast_ref::<gtk_dom::gtk::Label>()
        .expect("text-label should be backed by gtk::Label");
    assert_eq!(l.label().as_str(), "hello");
}

fn text_create_empty() {
    let tree = gtk_dom::layout::new_tree();
    let t = Element::create_text(&tree, "");
    let l = t
        .as_node()
        .widget()
        .downcast_ref::<gtk_dom::gtk::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "");
}

fn text_create_multiline_preserves_newlines() {
    let tree = gtk_dom::layout::new_tree();
    let t = Element::create_text(&tree, "line one\nline two\nline three");
    let l = t
        .as_node()
        .widget()
        .downcast_ref::<gtk_dom::gtk::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "line one\nline two\nline three");
}

fn text_set_text_updates_value() {
    let tree = gtk_dom::layout::new_tree();
    let t = Element::create_text(&tree, "before");
    t.set_text("after");
    let l = t
        .as_node()
        .widget()
        .downcast_ref::<gtk_dom::gtk::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "after");
}

fn placeholder_create_is_invisible() {
    let tree = gtk_dom::layout::new_tree();
    let p = Element::create_placeholder(&tree);

    let widget = p.as_node().widget();
    // Placeholders shouldn't take any layout space — gtk's
    // `set_visible(false)` removes them from layout entirely.
    assert!(!widget.is_visible());
}

fn placeholder_backed_by_label_so_children_error() {
    let tree = gtk_dom::layout::new_tree();
    // Implementation detail: we use gtk::Label rather than gtk::Box
    // for placeholders so that a `placeholder.append(child)` attempt
    // would fail at the GTK type level rather than silently mounting
    // an invisible child.
    let p = Element::create_placeholder(&tree);
    assert!(
        p.as_node().widget().is::<gtk_dom::gtk::Label>(),
        "Placeholder should be a gtk::Label so it can't accept children"
    );
}

fn main() {
    common::run_tests(&[
        ("text_create_basic", text_create_basic),
        ("text_create_empty", text_create_empty),
        (
            "text_create_multiline_preserves_newlines",
            text_create_multiline_preserves_newlines,
        ),
        ("text_set_text_updates_value", text_set_text_updates_value),
        ("placeholder_create_is_invisible", placeholder_create_is_invisible),
        (
            "placeholder_backed_by_label_so_children_error",
            placeholder_backed_by_label_so_children_error,
        ),
    ]);
}
