//! Regression tests for `AddAnyAttr<Dom>` on cocoa builders.
//!
//! Phase 9 minimal AddAnyAttr port — the macro-emitted
//! `MyComponent(props).add_any_attr((on_attr,))` path that lets users
//! attach `on:click` handlers directly to custom components.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::{
    cocoa::element::button,
    event_macos::{on, click},
    Dom,
};
use objc2_app_kit::NSControl;
use renderer::view::{AddAnyAttr, Render};
use std::sync::{Arc, Mutex};

/// REGRESSION: a button.add_any_attr((on(click, handler),)) call
/// must wire the handler so synthetic action firing invokes it.
fn add_any_attr_routes_on_click_to_button() {
    let _mtm = common::test_mtm();
    let _ = cocoa_dom::spawner::init();
    let owner = reactive_graph::owner::Owner::new();
    owner.with(|| {
        let fired = Arc::new(Mutex::new(0));
        let fired_clone = fired.clone();

        // Imitate what the macro emits for `<MyButton on:click=…>`:
        // construct the component's view (a bare button), then call
        // .add_any_attr((on(click, handler),)).
        let view = button()
            .title("OK")
            .add_any_attr((on(click, move |_: ()| {
                *fired_clone.lock().unwrap() += 1;
            }),));

        let st = view.build();

        // Synthesise a click via the test helper.
        let any: &objc2::runtime::AnyObject = st.el.ns_view().as_ref();
        let control: &NSControl = any
            .downcast_ref::<NSControl>()
            .expect("button view is an NSControl");
        common::fire_action(control);

        assert_eq!(*fired.lock().unwrap(), 1, "handler didn't fire");
    });
}

fn main() {
    common::run_tests(&[(
        "add_any_attr_routes_on_click_to_button",
        add_any_attr_routes_on_click_to_button,
    )]);
}
