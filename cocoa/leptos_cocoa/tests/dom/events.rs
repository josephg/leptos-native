//! Event-wiring tests.
//!
//! Exercises `CocoaNode::on_click` / `on_action` / `on_text_change` /
//! `on_text_end_editing` via the synthetic-action helpers in
//! `common`. No window or run loop required — target/action and
//! NSControlTextEditingDelegate notifications are dispatched
//! synchronously via the ObjC runtime / NSNotificationCenter.

#![cfg(target_os = "macos")]

mod common;

use objc2::runtime::AnyObject;
use objc2_app_kit::{NSControl, NSTextField};
use std::cell::Cell;
use std::rc::Rc;
use leptos_cocoa::dom::CocoaElem;
// ---------------------------------------------------------------------
// on_click — buttons + popups (NSButton subtree)
// ---------------------------------------------------------------------

fn on_click_fires_on_button() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_click(move || c.set(c.get() + 1));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let control = any.downcast_ref::<NSControl>().unwrap();
    common::fire_action(control);
    common::fire_action(control);

    assert_eq!(count.get(), 2);
}

fn on_click_on_label_is_no_op() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_label().0;
    // Should silently no-op (label isn't NSButton). Just verify no
    // panic; we don't fire action because there's nothing wired.
    el.on_click(|| panic!("must not fire"));
}

// ---------------------------------------------------------------------
// on_action — works for any NSControl (slider, popup, button)
// ---------------------------------------------------------------------

fn on_action_fires_on_slider() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_slider().0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_action(move || c.set(c.get() + 1));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let control = any.downcast_ref::<NSControl>().unwrap();
    common::fire_action(control);

    assert_eq!(count.get(), 1, "slider on_action should fire");
}

fn on_action_fires_on_popup() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_pop_up_button().0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_action(move || c.set(c.get() + 1));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let control = any.downcast_ref::<NSControl>().unwrap();
    common::fire_action(control);

    assert_eq!(count.get(), 1);
}

fn on_action_on_view_is_no_op() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_container();
    el.on_action(|| panic!("must not fire"));
}

// Regression guard: on_click on a slider would silently drop because
// NSSlider isn't a subclass of NSButton (both extend NSControl
// directly). We hit this bug once; this test prevents recurrence.
fn on_click_on_slider_silently_drops_no_panic() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_slider().0;
    // on_click expects NSButton; slider isn't one. Silent no-op.
    el.on_click(|| panic!("must not fire on slider via on_click"));
    // Action target is None; can't fire_action without panicking,
    // so we just confirm the install itself didn't panic.
}

// ---------------------------------------------------------------------
// Repeated installs — panic on NSControl double-install
// ---------------------------------------------------------------------

fn second_on_click_panics() {
    let _mtm = common::test_mtm();
    let result = std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| {
            let el = CocoaElem::create_button().0;
            el.on_click(|| {});
            // Second install on the same control panics rather
            // than silently overwriting the first.
            el.on_click(|| {});
        }),
    );
    let payload =
        result.expect_err("expected second on_click to panic");
    let msg = payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>");
    assert!(
        msg.contains("on_control_action called twice"),
        "panic message should explain duplicate install; got: {msg}"
    );
}

// ---------------------------------------------------------------------
// on_text_change — TextFieldDelegate fan-out
// ---------------------------------------------------------------------

fn on_text_change_fires_on_text_field() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let captured = Rc::new(Cell::new(String::new()));
    let c = captured.clone();
    el.on_text_change(move |v| c.set(v));

    // Set the string value programmatically, then post the
    // notification (mirrors what AppKit does on real keystrokes).
    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    field.setStringValue(&objc2_foundation::NSString::from_str("typed"));
    common::fire_text_did_change(field);

    assert_eq!(captured.take(), "typed");
}

fn on_text_change_on_button_is_no_op() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    el.on_text_change(|_| panic!("must not fire"));
}

fn multiple_on_text_change_fan_out() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let calls = Rc::new(Cell::new(0));
    let last_a = Rc::new(Cell::new(String::new()));
    let last_b = Rc::new(Cell::new(String::new()));

    {
        let c = calls.clone();
        let l = last_a.clone();
        el.on_text_change(move |v| {
            c.set(c.get() + 1);
            l.set(v);
        });
    }
    {
        let c = calls.clone();
        let l = last_b.clone();
        el.on_text_change(move |v| {
            c.set(c.get() + 1);
            l.set(v);
        });
    }

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    field.setStringValue(&objc2_foundation::NSString::from_str("hi"));
    common::fire_text_did_change(field);

    assert_eq!(calls.get(), 2, "both handlers fan out");
    assert_eq!(last_a.take(), "hi");
    assert_eq!(last_b.take(), "hi");
}

// ---------------------------------------------------------------------
// on_text_end_editing — coexists with on_text_change
// ---------------------------------------------------------------------

fn on_text_end_editing_fires_on_commit() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let captured = Rc::new(Cell::new(String::new()));
    let c = captured.clone();
    el.on_text_end_editing(move |v| c.set(v));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    field.setStringValue(&objc2_foundation::NSString::from_str("done"));
    common::fire_text_did_end_editing(field);

    assert_eq!(captured.take(), "done");
}

// ---------------------------------------------------------------------
// Focus / blur — controlTextDidBeginEditing / EndEditing
// ---------------------------------------------------------------------

fn on_text_focus_fires_on_begin_editing() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    el.on_text_focus(move || c.set(c.get() + 1));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_begin_editing(field);
    assert_eq!(calls.get(), 1);
}

fn on_text_blur_fires_on_end_editing() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    el.on_text_blur(move || c.set(c.get() + 1));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_end_editing(field);
    assert_eq!(calls.get(), 1);
}

fn on_change_and_on_blur_both_fire_on_commit() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let changes = Rc::new(Cell::new(0));
    let blurs = Rc::new(Cell::new(0));
    let last_change = Rc::new(Cell::new(String::new()));
    {
        let c = changes.clone();
        let l = last_change.clone();
        el.on_text_end_editing(move |v| {
            c.set(c.get() + 1);
            l.set(v);
        });
    }
    {
        let b = blurs.clone();
        el.on_text_blur(move || b.set(b.get() + 1));
    }

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    field.setStringValue(&objc2_foundation::NSString::from_str("done"));
    common::fire_text_did_end_editing(field);

    assert_eq!(changes.get(), 1, "change should fire");
    assert_eq!(blurs.get(), 1, "blur should fire alongside change");
    assert_eq!(last_change.take(), "done");
}

fn on_text_focus_on_button_is_no_op() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    el.on_text_focus(|| panic!("must not fire"));
}

// ---------------------------------------------------------------------
// Keydown / keyup — doCommandBySelector pipeline
// ---------------------------------------------------------------------

fn on_text_keydown_fires_on_enter() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let captured = Rc::new(Cell::new(None::<String>));
    let c = captured.clone();
    el.on_text_keydown(move |ev| c.set(Some(ev.key)));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_command(field, sel!(insertNewline:));

    assert_eq!(captured.take(), Some("Enter".to_string()));
}

fn on_text_keydown_fires_on_escape() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let captured = Rc::new(Cell::new(None::<u32>));
    let c = captured.clone();
    el.on_text_keydown(move |ev| c.set(Some(ev.key_code)));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_command(field, sel!(cancelOperation:));

    assert_eq!(captured.take(), Some(27));
}

fn on_text_keyup_fires_on_command_keys() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let captured = Rc::new(Cell::new(None::<String>));
    let c = captured.clone();
    el.on_text_keyup(move |ev| c.set(Some(ev.key)));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_command(field, sel!(insertTab:));

    assert_eq!(captured.take(), Some("Tab".to_string()));
}

fn keydown_and_keyup_both_fire_on_same_notification() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let down = Rc::new(Cell::new(0));
    let up = Rc::new(Cell::new(0));
    {
        let d = down.clone();
        el.on_text_keydown(move |_| d.set(d.get() + 1));
    }
    {
        let u = up.clone();
        el.on_text_keyup(move |_| u.set(u.get() + 1));
    }

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_command(field, sel!(insertNewline:));

    assert_eq!(down.get(), 1, "keydown fires on doCommand");
    assert_eq!(up.get(), 1, "keyup fires on the same doCommand");
}

fn unknown_command_selector_does_not_fire() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    el.on_text_keydown(|_| panic!("must not fire on unknown selector"));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    // `noop:` is a valid AppKit selector but not in our mapping.
    common::fire_text_did_command(field, sel!(noop:));
}

fn arrow_keys_map_to_web_names() {
    use objc2::sel;
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let names = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let n = names.clone();
    el.on_text_keydown(move |ev| n.borrow_mut().push(ev.key));

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    common::fire_text_did_command(field, sel!(moveUp:));
    common::fire_text_did_command(field, sel!(moveDown:));
    common::fire_text_did_command(field, sel!(moveLeft:));
    common::fire_text_did_command(field, sel!(moveRight:));

    assert_eq!(
        *names.borrow(),
        vec!["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"]
    );
}

fn keydown_on_button_is_no_op() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_button().0;
    el.on_text_keydown(|_| panic!("must not fire on button"));
}

fn on_change_and_on_input_coexist() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_text_field().0;
    let inputs = Rc::new(Cell::new(0));
    let changes = Rc::new(Cell::new(0));

    {
        let i = inputs.clone();
        el.on_text_change(move |_| i.set(i.get() + 1));
    }
    {
        let c = changes.clone();
        el.on_text_end_editing(move |_| c.set(c.get() + 1));
    }

    let __nv = el.ns_view();
    let any: &AnyObject = __nv.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    field.setStringValue(&objc2_foundation::NSString::from_str("x"));
    common::fire_text_did_change(field);
    common::fire_text_did_change(field);
    common::fire_text_did_end_editing(field);

    assert_eq!(inputs.get(), 2, "input fires per change notification");
    assert_eq!(changes.get(), 1, "change fires once on commit");
}

// ---------------------------------------------------------------------
// Element value getters (slider, popup, checkbox)
// ---------------------------------------------------------------------

fn slider_double_value_round_trips() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_slider().0;
    el.set_slider_min(0.0);
    el.set_slider_max(100.0);
    el.set_double_value(42.5);
    assert!((el.double_value() - 42.5).abs() < 1e-9);
}

fn popup_items_and_selection() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_pop_up_button().0;
    let items: Vec<String> =
        ["Alpha", "Beta", "Gamma"].into_iter().map(String::from).collect();
    el.set_popup_items(&items);
    el.set_popup_selection(2);
    assert_eq!(el.popup_selection(), 2);
}

fn checkbox_checked_round_trips() {
    let _mtm = common::test_mtm();
    let el = CocoaElem::create_checkbox().0;
    assert!(!el.checked());
    el.set_checked(true);
    assert!(el.checked());
}

fn main() {
    common::run_tests(&[
        // on_click
        ("on_click_fires_on_button", on_click_fires_on_button),
        ("on_click_on_label_is_no_op", on_click_on_label_is_no_op),
        // on_action
        ("on_action_fires_on_slider", on_action_fires_on_slider),
        ("on_action_fires_on_popup", on_action_fires_on_popup),
        ("on_action_on_view_is_no_op", on_action_on_view_is_no_op),
        (
            "on_click_on_slider_silently_drops_no_panic",
            on_click_on_slider_silently_drops_no_panic,
        ),
        // Repeated installs
        ("second_on_click_panics", second_on_click_panics),
        // Text change
        (
            "on_text_change_fires_on_text_field",
            on_text_change_fires_on_text_field,
        ),
        (
            "on_text_change_on_button_is_no_op",
            on_text_change_on_button_is_no_op,
        ),
        ("multiple_on_text_change_fan_out", multiple_on_text_change_fan_out),
        // End editing
        (
            "on_text_end_editing_fires_on_commit",
            on_text_end_editing_fires_on_commit,
        ),
        ("on_change_and_on_input_coexist", on_change_and_on_input_coexist),
        // Focus / blur
        ("on_text_focus_fires_on_begin_editing", on_text_focus_fires_on_begin_editing),
        ("on_text_blur_fires_on_end_editing", on_text_blur_fires_on_end_editing),
        ("on_change_and_on_blur_both_fire_on_commit", on_change_and_on_blur_both_fire_on_commit),
        ("on_text_focus_on_button_is_no_op", on_text_focus_on_button_is_no_op),
        // Keydown / keyup
        ("on_text_keydown_fires_on_enter", on_text_keydown_fires_on_enter),
        ("on_text_keydown_fires_on_escape", on_text_keydown_fires_on_escape),
        ("on_text_keyup_fires_on_command_keys", on_text_keyup_fires_on_command_keys),
        ("keydown_and_keyup_both_fire_on_same_notification", keydown_and_keyup_both_fire_on_same_notification),
        ("unknown_command_selector_does_not_fire", unknown_command_selector_does_not_fire),
        ("arrow_keys_map_to_web_names", arrow_keys_map_to_web_names),
        ("keydown_on_button_is_no_op", keydown_on_button_is_no_op),
        // Value getters
        ("slider_double_value_round_trips", slider_double_value_round_trips),
        ("popup_items_and_selection", popup_items_and_selection),
        ("checkbox_checked_round_trips", checkbox_checked_round_trips),
    ]);
}
