//! Regression tests for `AddAnyAttr<Dom>` on cocoa builders — the
//! macro-emitted `MyComponent(props).add_any_attr((on_attr,))` path
//! that lets users attach `on:click` handlers directly to custom
//! components.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::spawner;
use leptos_cocoa::{
    cocoa::element::button,
    event_macos::{click, on},
    CocoaDom,
};
use objc2_app_kit::NSControl;
use leptos_native::renderer::view::{AddAnyAttr, Render};
use std::sync::{Arc, Mutex};

/// REGRESSION: a button.add_any_attr((on(click, handler),)) call
/// must wire the handler so synthetic action firing invokes it.
fn add_any_attr_routes_on_click_to_button() {
    let _mtm = common::test_mtm();
    let _ = spawner::init().unwrap();
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
        let view = st.el.ns_view();
        let any: &objc2::runtime::AnyObject = view.as_ref();
        let control: &NSControl = any
            .downcast_ref::<NSControl>()
            .expect("button view is an NSControl");
        common::fire_action(control);

        assert_eq!(*fired.lock().unwrap(), 1, "handler didn't fire");
    });
}

/// Per the failure-mode hierarchy in CLAUDE.md, a spread on a
/// branching wrapper (closure / Either / Option / Vec / ErrorBoundary)
/// MUST panic, not silently drop. Regression test for that contract.
fn add_any_attr_panics_on_reactive_closure() {
    let _mtm = common::test_mtm();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let view = move || button().title("hi");
        let _ = <_ as AddAnyAttr<CocoaDom>>::add_any_attr(
            view,
            (on(click, |_: ()| {}),),
        );
    }));
    assert!(
        result.is_err(),
        "expected panic on add_any_attr on a reactive closure"
    );
    let payload = result.unwrap_err();
    let msg = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&'static str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(
        msg.contains("reactive closure") || msg.contains("Show"),
        "panic message should name the kind, got: {msg}"
    );
}

/// `<Component on:click=…>` where Component returns just a String —
/// no element to install on. Must panic.
fn add_any_attr_panics_on_string() {
    let _mtm = common::test_mtm();
    let result = std::panic::catch_unwind(|| {
        let s: String = "hello".into();
        let _ = <String as AddAnyAttr<CocoaDom>>::add_any_attr(
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
