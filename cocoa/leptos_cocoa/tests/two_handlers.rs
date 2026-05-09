//! Two-handler-on-one-NSControl panic regression tests at the
//! `leptos_cocoa` layer.
//!
//! NSControl has a single target/action slot; `cocoa_dom::event`
//! panics if a second handler is installed. The lower-level test
//! lives in `cocoa_dom/tests/events.rs::second_on_click_panics`. This
//! file pins the behavior at the `leptos_cocoa` builder layer — the
//! path that user `<button on:click=…>` code actually exercises.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::cocoa::element::button;
use leptos_cocoa::event_macos::{click, on};
use reactive_graph::owner::Owner;
use renderer::view::{AddAnyAttr, Render};

fn with_reactive_scope<F: FnOnce()>(f: F) {
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(f);
}

/// Two `add_any_attr((on(click, …),))` calls on the same builder
/// must panic at build time — the second handler installation
/// detects the existing target/action and aborts.
fn two_on_click_via_add_any_attr_panics() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = button()
            .title("OK")
            .add_any_attr((on(click, |_: ()| {}),))
            .add_any_attr((on(click, |_: ()| {}),));

        // Building should panic when the second on:click is
        // installed during build.
        let result = std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(|| {
                let _ = view.build();
            }),
        );
        assert!(
            result.is_err(),
            "expected panic when installing two on:click handlers via \
             add_any_attr"
        );
    });
}

/// Single on:click handler should NOT panic — sanity check that the
/// panic above isn't being raised on every build.
fn one_on_click_does_not_panic() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = button()
            .title("OK")
            .add_any_attr((on(click, |_: ()| {}),));
        let _ = view.build();
    });
}

fn main() {
    common::run_tests(&[
        (
            "two_on_click_via_add_any_attr_panics",
            two_on_click_via_add_any_attr_panics,
        ),
        ("one_on_click_does_not_panic", one_on_click_does_not_panic),
    ]);
}
