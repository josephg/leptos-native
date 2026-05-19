//! Smoke tests for `Element::create` per tag.
//!
//! Verifies each tag string maps to a GTK widget class.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{gtk::prelude::*, Element};

fn view_is_a_box() {
    let tree = gtk_dom::layout::new_tree();
    // `<view>` is a generic flexbox container, backed by gtk::Box
    // (with our TaffyLayout swapped in once mounted).
    let el = Element::create(&tree, "view");
    assert!(el.widget().is::<gtk_dom::gtk::Box>());
}

fn vstack_is_a_box() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "vstack");
    assert!(el.widget().is::<gtk_dom::gtk::Box>());
}

fn hstack_is_a_box() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "hstack");
    assert!(el.widget().is::<gtk_dom::gtk::Box>());
}

fn stack_is_a_box() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "stack");
    assert!(el.widget().is::<gtk_dom::gtk::Box>());
}

fn button_is_gtk_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "button");
    assert!(el.widget().is::<gtk_dom::gtk::Button>());
}

fn checkbox_is_gtk_check_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "checkbox");
    assert!(el.widget().is::<gtk_dom::gtk::CheckButton>());
}

fn label_is_gtk_label() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "label");
    assert!(el.widget().is::<gtk_dom::gtk::Label>());
}

fn label_wraps_by_default() {
    let tree = gtk_dom::layout::new_tree();
    // gtk::Label::wrap defaults to false, but we flip it on at
    // construction so multi-line text behaves like cocoa's
    // wrappingLabelWithString:.
    let el = Element::create(&tree, "label");
    let l = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Label>()
        .unwrap();
    assert!(l.wraps(), "label should wrap by default");
}

fn text_field_is_gtk_entry() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "text_field");
    assert!(el.widget().is::<gtk_dom::gtk::Entry>());
    assert!(
        !el.widget().is::<gtk_dom::gtk::PasswordEntry>(),
        "plain text_field shouldn't be a password entry"
    );
}

fn secure_text_field_is_password_entry() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "secure_text_field");
    assert!(el.widget().is::<gtk_dom::gtk::PasswordEntry>());
}

fn slider_is_gtk_scale() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "slider");
    let v = el.widget();
    assert!(v.is::<gtk_dom::gtk::Scale>());
    let s = v.downcast_ref::<gtk_dom::gtk::Scale>().unwrap();
    // We disable the value display since cocoa sliders don't show one
    // either — keep cross-platform parity.
    assert!(!s.draws_value());
}

fn pop_up_button_is_drop_down() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "pop_up_button");
    assert!(el.widget().is::<gtk_dom::gtk::DropDown>());
}

fn unknown_tag_falls_back_to_view() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create(&tree, "totally_made_up_zzz");
    assert!(el.widget().is::<gtk_dom::gtk::Box>());
    assert!(!el.widget().is::<gtk_dom::gtk::Button>());
}

fn main() {
    common::run_tests(&[
        ("view_is_a_box", view_is_a_box),
        ("vstack_is_a_box", vstack_is_a_box),
        ("hstack_is_a_box", hstack_is_a_box),
        ("stack_is_a_box", stack_is_a_box),
        ("button_is_gtk_button", button_is_gtk_button),
        ("checkbox_is_gtk_check_button", checkbox_is_gtk_check_button),
        ("label_is_gtk_label", label_is_gtk_label),
        ("label_wraps_by_default", label_wraps_by_default),
        ("text_field_is_gtk_entry", text_field_is_gtk_entry),
        (
            "secure_text_field_is_password_entry",
            secure_text_field_is_password_entry,
        ),
        ("slider_is_gtk_scale", slider_is_gtk_scale),
        ("pop_up_button_is_drop_down", pop_up_button_is_drop_down),
        ("unknown_tag_falls_back_to_view", unknown_tag_falls_back_to_view),
        ]);
}
