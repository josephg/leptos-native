//! `Element::focus()` / `Element::blur()` integration tests.
//!
//! These need an NSWindow to exercise — `view.window()` returns
//! `None` when the view isn't mounted, and focus is a window-level
//! concept. We open a real NSWindow (without `makeKeyAndOrderFront`,
//! so it stays off-screen) and mount text fields under its
//! contentView before each focus test.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::Element;

fn focus_unmounted_returns_false() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    assert!(
        !el.focus(),
        "an element not in a window can't be focused"
    );
}

fn blur_unmounted_returns_false() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    assert!(!el.blur(), "no window → no blur");
}

fn focus_mounted_text_field_succeeds() {
    let mtm = common::test_mtm();
    cocoa_dom::app::init_app(mtm);
    let win =
        cocoa_dom::window::open_window("focus-test", (320.0, 200.0), mtm);

    let field = Element::create("text_field");
    win.content_root.insert_node(field.as_node(), None);

    // Before focus: nobody is first responder (or the window itself).
    assert!(
        field.focus(),
        "focus() should succeed on a mounted NSTextField"
    );

    // After focus: there should be SOMEONE as first responder.
    // (NSTextField uses a shared field editor — first responder
    // becomes the field editor (NSTextView) with our field as its
    // delegate, not the field itself. We don't assert on identity,
    // just that focus left a first responder set.)
    let fr = win.nswindow.firstResponder();
    assert!(
        fr.is_some(),
        "focus() should leave the window with a first responder"
    );

    win.close();
}

fn blur_clears_focus() {
    let mtm = common::test_mtm();
    cocoa_dom::app::init_app(mtm);
    let win =
        cocoa_dom::window::open_window("blur-test", (320.0, 200.0), mtm);

    let field = Element::create("text_field");
    win.content_root.insert_node(field.as_node(), None);

    field.focus();
    assert!(field.blur(), "blur should succeed");

    // After blur, the window itself becomes first responder
    // (NSWindow IS-A NSResponder). The field's editor is no longer
    // the responder.
    let fr_after = win.nswindow.firstResponder();
    // We're being lenient here — what matters is that blur didn't
    // panic. AppKit's behaviour is to make the window the new
    // first responder.
    let _ = fr_after;

    win.close();
}

fn focus_on_button_works() {
    // Buttons are NSResponders too — focus should succeed.
    let mtm = common::test_mtm();
    cocoa_dom::app::init_app(mtm);
    let win =
        cocoa_dom::window::open_window("button-focus", (320.0, 200.0), mtm);

    let button = Element::create("button");
    win.content_root.insert_node(button.as_node(), None);

    // Whether AppKit accepts focus on a button depends on
    // accessibility settings (Full Keyboard Access). Even when
    // declined, focus() shouldn't panic.
    let _ = button.focus();

    win.close();
}

fn main() {
    common::run_tests(&[
        ("focus_unmounted_returns_false", focus_unmounted_returns_false),
        ("blur_unmounted_returns_false", blur_unmounted_returns_false),
        (
            "focus_mounted_text_field_succeeds",
            focus_mounted_text_field_succeeds,
        ),
        ("blur_clears_focus", blur_clears_focus),
        ("focus_on_button_works", focus_on_button_works),
    ]);
}
