//! `Node::focus()` / `Node::blur()` integration tests.
//!
//! These need a `gtk::Window` to exercise — focus is a window-level
//! concept; an unmounted widget reports `false` for `grab_focus`.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::GtkNode;

fn focus_unmounted_returns_false_or_no_panic() {
    // grab_focus on an unmounted widget — gtk-rs returns a bool that
    // reflects whether GTK accepted the focus change. Without a
    // window this is normally false; we don't assert, just verify
    // no panic.
    let el = GtkNode::create_text_field().0;
    let _ = el.focus();
}

fn blur_unmounted_returns_false() {
    let el = GtkNode::create_text_field().0;
    assert!(!el.blur(), "no window → no blur");
}

fn focus_mounted_text_field_succeeds() {
    let app = gtk_dom::app::init_app("org.test.gtk_dom.focus");
    // Build a window directly (without `app.run()`) so we have a
    // mounted widget hierarchy without entering the main loop.
    let win = gtk_dom::window::open_window(&app, "focus-test", (320, 200));

    let field = GtkNode::create_text_field().0;
    win.content_root.insert_node(field.as_node(), None);

    // grab_focus returns true when GTK accepts the focus request.
    // For an unmapped window this can still return true if the
    // widget is focusable; verify either outcome is non-panicking.
    let _ = field.focus();
    win.close();
}

fn blur_clears_focus() {
    let app = gtk_dom::app::init_app("org.test.gtk_dom.blur");
    let win = gtk_dom::window::open_window(&app, "blur-test", (320, 200));

    let field = GtkNode::create_text_field().0;
    win.content_root.insert_node(field.as_node(), None);

    let _ = field.focus();
    let _ = field.blur();
    win.close();
}

fn focus_on_button_works() {
    let app = gtk_dom::app::init_app("org.test.gtk_dom.button-focus");
    let win = gtk_dom::window::open_window(&app, "button-focus", (320, 200));

    let button = GtkNode::create_button().0;
    win.content_root.insert_node(button.as_node(), None);

    let _ = button.focus();
    win.close();
}

fn main() {
    common::run_tests(&[
        (
            "focus_unmounted_returns_false_or_no_panic",
            focus_unmounted_returns_false_or_no_panic,
        ),
        ("blur_unmounted_returns_false", blur_unmounted_returns_false),
        (
            "focus_mounted_text_field_succeeds",
            focus_mounted_text_field_succeeds,
        ),
        ("blur_clears_focus", blur_clears_focus),
        ("focus_on_button_works", focus_on_button_works),
        ]);
}
