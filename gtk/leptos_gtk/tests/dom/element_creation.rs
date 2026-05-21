//! Smoke tests for `Node::create` per tag.
//!
//! Verifies each tag string maps to a GTK widget class.

#![cfg(feature = "gtk")]

mod common;

use leptos_gtk::dom::GtkElem;
use leptos_gtk::gtk4::prelude::*;

fn view_is_a_box() {
    // `<view>` is a generic flexbox container, backed by gtk::Box
    // (with our TaffyLayout swapped in once mounted).
    let el = GtkElem::create_stack();
    assert!(el.widget().is::<gtk4::Box>());
}

fn vstack_is_a_box() {
    let el = GtkElem::create_vstack();
    assert!(el.widget().is::<gtk4::Box>());
}

fn hstack_is_a_box() {
    let el = GtkElem::create_hstack();
    assert!(el.widget().is::<gtk4::Box>());
}

fn stack_is_a_box() {
    let el = GtkElem::create_stack();
    assert!(el.widget().is::<gtk4::Box>());
}

fn button_is_gtk_button() {
    let el = GtkElem::create_button().0;
    assert!(el.widget().is::<gtk4::Button>());
}

fn checkbox_is_gtk_check_button() {
    let el = GtkElem::create_checkbox().0;
    assert!(el.widget().is::<gtk4::CheckButton>());
}

fn label_is_gtk_label() {
    let el = GtkElem::create_label().0;
    assert!(el.widget().is::<gtk4::Label>());
}

fn label_wraps_by_default() {
    // gtk::Label::wrap defaults to false, but we flip it on at
    // construction so multi-line text behaves like cocoa's
    // wrappingLabelWithString:.
    let el = GtkElem::create_label().0;
    let __w = el
        .widget();
    let l = __w.downcast_ref::<gtk4::Label>()
        .unwrap();
    assert!(l.wraps(), "label should wrap by default");
}

fn text_field_is_gtk_entry() {
    let el = GtkElem::create_text_field().0;
    assert!(el.widget().is::<gtk4::Entry>());
    assert!(
        !el.widget().is::<gtk4::PasswordEntry>(),
        "plain text_field shouldn't be a password entry"
    );
}

fn secure_text_field_is_password_entry() {
    let el = GtkElem::create_secure_text_field().0;
    assert!(el.widget().is::<gtk4::PasswordEntry>());
}

fn slider_is_gtk_scale() {
    let el = GtkElem::create_slider().0;
    let v = el.widget();
    assert!(v.is::<gtk4::Scale>());
    let s = v.downcast_ref::<gtk4::Scale>().unwrap();
    // We disable the value display since cocoa sliders don't show one
    // either — keep cross-platform parity.
    assert!(!s.draws_value());
}

fn pop_up_button_is_drop_down() {
    let el = GtkElem::create_pop_up_button().0;
    assert!(el.widget().is::<gtk4::DropDown>());
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
        ]);
}
