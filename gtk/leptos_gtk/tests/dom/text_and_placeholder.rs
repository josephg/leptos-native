//! Tests for the text-label (`Node::create_text`) and placeholder
//! (`Node::create_placeholder`) constructors on `Element`.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::{gtk4::prelude::*, dom::GtkElem};
use leptos_gtk::dom::GtkNodeExt;

fn text_create_basic() {
    let t = GtkElem::create_text("hello");

    let __w = t
        .widget();
    let l = __w.downcast_ref::<gtk4::Label>()
        .expect("text-label should be backed by gtk::Label");
    assert_eq!(l.label().as_str(), "hello");
}

fn text_create_empty() {
    let t = GtkElem::create_text("");
    let __w = t
        .widget();
    let l = __w.downcast_ref::<gtk4::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "");
}

fn text_create_multiline_preserves_newlines() {
    let t = GtkElem::create_text("line one\nline two\nline three");
    let __w = t
        .widget();
    let l = __w.downcast_ref::<gtk4::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "line one\nline two\nline three");
}

fn text_set_text_updates_value() {
    let t = GtkElem::create_text("before");
    t.set_text("after");
    let __w = t
        .widget();
    let l = __w.downcast_ref::<gtk4::Label>()
        .unwrap();
    assert_eq!(l.label().as_str(), "after");
}

fn placeholder_create_is_invisible() {
    let p = GtkElem::create_placeholder();

    let widget = p.widget();
    // Placeholders shouldn't take any layout space — gtk's
    // `set_visible(false)` removes them from layout entirely.
    assert!(!widget.is_visible());
}

fn placeholder_backed_by_label_so_children_error() {
    // Implementation detail: we use gtk::Label rather than gtk::Box
    // for placeholders so that a `placeholder.append(child)` attempt
    // would fail at the GTK type level rather than silently mounting
    // an invisible child.
    let p = GtkElem::create_placeholder();
    assert!(
        p.widget().is::<gtk4::Label>(),
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
