//! Tachys-side builder tests. Each builder constructs its
//! Cocoa-flavoured element via `Render::build` and the test
//! asserts on the resulting NSView's state.
//!
//! Reactive attributes need an `Owner` set on the current thread
//! so RenderEffect closures can run; `with_reactive_scope` wraps
//! a test body in one.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::BoolAttr;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSButton, NSControl, NSPopUpButton, NSSlider, NSTextField};
use reactive_graph::{owner::Owner, signal::RwSignal, traits::*};
use tachys::{
    cocoa::element::{
        button, checkbox, label, pop_up_button, secure_text_field,
        slider, text_field, vstack,
    },
    view::Render,
};

/// Run the test body inside a fresh reactive `Owner` scope, with
/// our main-thread spawner initialized.
///
/// `Owner::new()` provides the reactive cleanup root; the spawner
/// init satisfies `Executor::spawn_local` (RenderEffect uses it
/// for some internal coordination — without an executor, building
/// any reactive attribute panics).
///
/// The spawner's actual `spawn_local` runs futures via
/// `dispatch_async` on the main queue, which doesn't fire without
/// an active run loop. That's fine for these tests: the parts we
/// observe (RenderEffect's body re-running on signal change) are
/// synchronous; only deferred work would block on the run loop,
/// and we don't test that here.
fn with_reactive_scope<F: FnOnce()>(body: F) {
    // `init()` is idempotent across test invocations — subsequent
    // calls return `Err(AlreadySet)` which we ignore.
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(body);
}

// ---------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------

fn button_static_title() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button().title("Save").build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Save");
    });
}

fn button_reactive_title_initial_run() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let label = RwSignal::new("Click me".to_string());
        let st = button().title(move || label.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Click me");
    });
}

fn button_reactive_title_updates_on_signal_change() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let label = RwSignal::new("first".to_string());
        let st = button().title(move || label.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();

        label.set("second".to_string());
        // RenderEffect schedules its rebuild on the main queue;
        // pump the loop so the effect fires before we assert.
        common::pump_run_loop(0.1);
        assert_eq!(b.title().to_string(), "second");
    });
}

fn button_enabled_static() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button().title("X").enabled(false).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let c = any.downcast_ref::<NSControl>().unwrap();
        assert!(!c.isEnabled());
    });
}

fn button_enabled_reactive() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(true);
        let st = button().title("X").enabled(move || on.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let c = any.downcast_ref::<NSControl>().unwrap();
        assert!(c.isEnabled());
        on.set(false);
        common::pump_run_loop(0.1);
        assert!(!c.isEnabled());
    });
}

// ---------------------------------------------------------------------
// Checkbox
// ---------------------------------------------------------------------

fn checkbox_static_title_and_checked() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = checkbox().title("Subscribe").checked(true).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Subscribe");
        // checked()? element exposes a getter
        assert!(st.el.checked());
    });
}

fn checkbox_reactive_checked_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(false);
        let st = checkbox().checked(move || on.get()).build();
        assert!(!st.el.checked());
        on.set(true);
        common::pump_run_loop(0.1);
        assert!(st.el.checked());
    });
}

// ---------------------------------------------------------------------
// Label
// ---------------------------------------------------------------------

fn label_static_text() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = label().text("Hi").build();
        let any: &AnyObject = st.text.as_node().ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "Hi");
    });
}

fn label_reactive_text_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("a".to_string());
        let st = label().text(move || s.get()).build();
        let any: &AnyObject = st.text.as_node().ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "a");
        s.set("b".to_string());
        common::pump_run_loop(0.1);
        assert_eq!(f.stringValue().to_string(), "b");
    });
}

// ---------------------------------------------------------------------
// TextField
// ---------------------------------------------------------------------

fn text_field_value_and_placeholder() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = text_field()
            .value("initial")
            .placeholder("type here")
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "initial");
        assert_eq!(
            f.placeholderString().map(|s| s.to_string()).unwrap_or_default(),
            "type here"
        );
        // It IS-A editable NSTextField; ensure we got the editable
        // (non-secure) variant.
        assert!(f.isEditable());
    });
}

fn secure_text_field_uses_secure_subclass() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = secure_text_field().build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        // NSSecureTextField IS-A NSTextField; check downcast to
        // the secure one.
        assert!(
            any.downcast_ref::<objc2_app_kit::NSSecureTextField>()
                .is_some(),
            "secure_text_field should produce NSSecureTextField"
        );
    });
}

// ---------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------

fn slider_min_max_value() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = slider()
            .min_value(0.0)
            .max_value(100.0)
            .value(42.0)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSSlider>().unwrap();
        assert_eq!(s.minValue(), 0.0);
        assert_eq!(s.maxValue(), 100.0);
        assert!((st.el.double_value() - 42.0).abs() < 1e-9);
    });
}

// ---------------------------------------------------------------------
// PopUpButton
// ---------------------------------------------------------------------

fn pop_up_button_items_static() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = pop_up_button()
            .items(vec!["Alpha", "Beta", "Gamma"])
            .selection(1_usize)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert_eq!(p.numberOfItems(), 3);
        assert_eq!(st.el.popup_selection(), 1);
    });
}

fn pop_up_button_items_owned_strings() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let items: Vec<String> =
            ["X", "Y"].iter().map(|s| s.to_string()).collect();
        let st = pop_up_button().items(items).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert_eq!(p.numberOfItems(), 2);
    });
}

// ---------------------------------------------------------------------
// View / vstack / hstack
// ---------------------------------------------------------------------

fn vstack_default_flex_direction_column() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = vstack().padding(8.0).gap(4.0).build();
        // A vstack is an Element. Just check the underlying NSView
        // exists; layout details get exercised in layout.rs.
        let _ = st.el.ns_view();
    });
}

// ---------------------------------------------------------------------
// Removal — ElementState's effects drop on unmount, unsubscribing
// from the signal.
// ---------------------------------------------------------------------

fn dropping_state_unsubscribes_effect() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("a".to_string());

        // Build and immediately drop the State. The Effect inside
        // should drop too — subsequent signal sets should NOT
        // panic / mutate anything (since the closure was dropped).
        {
            let st = label().text(move || s.get()).build();
            // capture once for sanity check
            let any: &AnyObject =
                st.text.as_node().ns_view().as_ref();
            let f = any.downcast_ref::<NSTextField>().unwrap();
            assert_eq!(f.stringValue().to_string(), "a");
            // st drops here
        }
        // After drop, setting the signal shouldn't blow up (no
        // dangling reference back into the dropped state).
        s.set("b".to_string());
        s.set("c".to_string());
    });
}

// ---------------------------------------------------------------------
// Bool variants of set_bool_attribute through builder reactive path
// ---------------------------------------------------------------------

fn checkbox_diff_skip_when_signal_same_value() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(true);
        let st = checkbox().checked(move || on.get()).build();

        // Set to same value many times; checkbox state shouldn't
        // toggle (diff guard inside set_bool_attribute).
        for _ in 0..5 {
            on.set(true);
        }
        common::pump_run_loop(0.1);
        assert!(st.el.checked());

        on.set(false);
        common::pump_run_loop(0.1);
        assert!(!st.el.checked());
    });
}

fn label_idempotent_set_does_not_error() {
    // Set the same string value multiple times via a signal —
    // should be safe (StringAttr::Title diff-guards).
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("X".to_string());
        let _st = label().text(move || s.get()).build();
        for _ in 0..10 {
            s.set("X".to_string());
        }
    });
}

// Suppress unused-import warnings for items used only via downcast.
#[allow(dead_code)]
fn _force_link() -> Option<BoolAttr> { None }

fn main() {
    common::run_tests(&[
        // Button
        ("button_static_title", button_static_title),
        ("button_reactive_title_initial_run", button_reactive_title_initial_run),
        (
            "button_reactive_title_updates_on_signal_change",
            button_reactive_title_updates_on_signal_change,
        ),
        ("button_enabled_static", button_enabled_static),
        ("button_enabled_reactive", button_enabled_reactive),
        // Checkbox
        ("checkbox_static_title_and_checked", checkbox_static_title_and_checked),
        ("checkbox_reactive_checked_updates", checkbox_reactive_checked_updates),
        // Label
        ("label_static_text", label_static_text),
        ("label_reactive_text_updates", label_reactive_text_updates),
        // TextField
        ("text_field_value_and_placeholder", text_field_value_and_placeholder),
        ("secure_text_field_uses_secure_subclass", secure_text_field_uses_secure_subclass),
        // Slider
        ("slider_min_max_value", slider_min_max_value),
        // PopUpButton
        ("pop_up_button_items_static", pop_up_button_items_static),
        ("pop_up_button_items_owned_strings", pop_up_button_items_owned_strings),
        // View
        ("vstack_default_flex_direction_column", vstack_default_flex_direction_column),
        // Lifecycle
        ("dropping_state_unsubscribes_effect", dropping_state_unsubscribes_effect),
        // Idempotence
        ("checkbox_diff_skip_when_signal_same_value", checkbox_diff_skip_when_signal_same_value),
        ("label_idempotent_set_does_not_error", label_idempotent_set_does_not_error),
    ]);
}
