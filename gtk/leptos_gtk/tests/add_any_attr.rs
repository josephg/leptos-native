//! `AddAnyAttr<Dom>` regression tests for gtk builders. Mirrors
//! `leptos_cocoa/tests/add_any_attr.rs`.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::gtk::prelude::*;
use leptos_gtk::{
    event_gtk::{click, on},
    gtk::element::button,
    Dom,
};
use renderer::view::{AddAnyAttr, Render};
use std::sync::{Arc, Mutex};

fn add_any_attr_routes_on_click_to_button() {
    let _ = gtk_dom::spawner::init();
    let owner = reactive_graph::owner::Owner::new();
    owner.with(|| {
        let fired = Arc::new(Mutex::new(0));
        let fired_clone = fired.clone();

        let view = button()
            .title("OK")
            .add_any_attr((on(click, move |_: ()| {
                *fired_clone.lock().unwrap() += 1;
            }),));

        let st = view.build();

        // GTK4: emit the `clicked` signal directly on the button.
        let __w = st
            .el
            .widget();
        let b = __w.downcast_ref::<gtk_dom::gtk::Button>()
            .unwrap();
        b.emit_clicked();

        assert_eq!(*fired.lock().unwrap(), 1, "handler didn't fire");
    });
}

fn add_any_attr_panics_on_reactive_closure() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let view = move || button().title("hi");
        let _ = <_ as AddAnyAttr<Dom>>::add_any_attr(
            view,
            (on(click, |_: ()| {}),),
        );
    }));
    assert!(
        result.is_err(),
        "expected panic on add_any_attr on a reactive closure"
    );
}

fn add_any_attr_panics_on_string() {
    let result = std::panic::catch_unwind(|| {
        let s: String = "hello".into();
        let _ = <String as AddAnyAttr<Dom>>::add_any_attr(
            s,
            (on(click, |_: ()| {}),),
        );
    });
    assert!(result.is_err(), "expected panic on add_any_attr on String");
}

fn main() {
    common::run_tests(&[
        (
            "add_any_attr_routes_on_click_to_button",
            add_any_attr_routes_on_click_to_button,
        ),
        (
            "add_any_attr_panics_on_reactive_closure",
            add_any_attr_panics_on_reactive_closure,
        ),
        (
            "add_any_attr_panics_on_string",
            add_any_attr_panics_on_string,
        ),
    ]);
}
