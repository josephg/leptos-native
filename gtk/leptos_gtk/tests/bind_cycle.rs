//! Bind-cycle protection regression tests. Mirrors
//! `leptos_cocoa/tests/bind_cycle.rs`.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{gtk::prelude::*, BoolAttr, Element};
use reactive_graph::owner::Owner;

fn with_reactive_scope<F: FnOnce()>(f: F) {
    let _ = gtk_dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

fn set_attribute_with_same_value_does_not_re_set() {
    with_reactive_scope(|| {
        let text_field = Element::create("text_field");

        text_field.set_attribute("value", "hello");
        let after_first = text_field
            .widget()
            .downcast_ref::<gtk_dom::gtk::Entry>()
            .unwrap()
            .text()
            .to_string();
        assert_eq!(after_first, "hello");

        text_field.set_attribute("value", "hello");

        let after_second = text_field
            .widget()
            .downcast_ref::<gtk_dom::gtk::Entry>()
            .unwrap()
            .text()
            .to_string();
        assert_eq!(after_second, "hello");
    });
}

fn set_bool_attribute_with_same_value_idempotent() {
    with_reactive_scope(|| {
        let checkbox = Element::create("checkbox");

        checkbox.set_bool_attribute(BoolAttr::Checked, true);
        let cb = checkbox
            .widget()
            .downcast_ref::<gtk_dom::gtk::CheckButton>()
            .unwrap();
        assert!(cb.is_active());

        checkbox.set_bool_attribute(BoolAttr::Checked, true);
        assert!(cb.is_active());
    });
}

fn main() {
    common::run_tests(&[
        (
            "set_attribute_with_same_value_does_not_re_set",
            set_attribute_with_same_value_does_not_re_set,
        ),
        (
            "set_bool_attribute_with_same_value_idempotent",
            set_bool_attribute_with_same_value_idempotent,
        ),
    ]);
}
