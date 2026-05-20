//! Event-wiring tests.
//!
//! Exercises `Element::on_click` / `on_action` / `on_text_change` /
//! `on_text_end_editing` via direct GTK signal emission. No window or
//! main loop required — `emit_by_name` synchronously dispatches.

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{gtk::prelude::*, Element};
use std::cell::Cell;
use std::rc::Rc;

// ---------------------------------------------------------------------
// on_click — buttons + checkbox + dropdown
// ---------------------------------------------------------------------

fn on_click_fires_on_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_click(move || c.set(c.get() + 1));

    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    b.emit_clicked();
    b.emit_clicked();

    assert_eq!(count.get(), 2);
}

fn on_click_on_label_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_label(&tree).0;
    el.on_click(|| panic!("must not fire"));
    // No way to "fire a click" on a label; the assertion is that
    // installing didn't panic and won't fire. Just verify by
    // dropping the element without crash.
}

fn on_click_on_checkbox_fires_on_toggle() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_click(move || c.set(c.get() + 1));

    // Toggling a CheckButton fires the `toggled` signal — our
    // `on_click` routes through that.
    let cb = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::CheckButton>()
        .unwrap();
    cb.set_active(true);
    cb.set_active(false);

    assert_eq!(count.get(), 2, "checkbox toggle should fire on_click");
}

// ---------------------------------------------------------------------
// on_action — works for any "value-changed" control
// ---------------------------------------------------------------------

fn on_action_fires_on_slider() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_slider(&tree).0;
    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_action(move || c.set(c.get() + 1));

    el.set_slider_min(0.0);
    el.set_slider_max(100.0);
    el.set_double_value(42.0);

    assert!(count.get() >= 1, "slider on_action should fire");
}

fn on_action_fires_on_dropdown() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_pop_up_button(&tree).0;
    el.set_popup_items(&["A".to_string(), "B".to_string(), "C".to_string()]);

    let count = Rc::new(Cell::new(0));
    let c = count.clone();
    el.on_action(move || c.set(c.get() + 1));

    el.set_popup_selection(2);

    assert!(count.get() >= 1, "dropdown on_action should fire");
}

fn on_action_on_view_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_stack(&tree);
    el.on_action(|| panic!("must not fire"));
}

// ---------------------------------------------------------------------
// on_text_change — Entry::changed
// ---------------------------------------------------------------------

fn on_text_change_fires_on_text_field() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    let captured = Rc::new(Cell::new(String::new()));
    let c = captured.clone();
    el.on_text_change(move |v| c.set(v));

    let entry = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    entry.set_text("typed");

    assert_eq!(captured.take(), "typed");
}

fn on_text_change_on_button_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.on_text_change(|_| panic!("must not fire"));
}

fn multiple_on_text_change_fan_out() {
    let tree = gtk_dom::layout::new_tree();
    // Each `on_text_change` install adds another connect_changed
    // signal connection — they all fire.
    let el = Element::create_text_field(&tree).0;
    let calls = Rc::new(Cell::new(0));
    {
        let c = calls.clone();
        el.on_text_change(move |_| c.set(c.get() + 1));
    }
    {
        let c = calls.clone();
        el.on_text_change(move |_| c.set(c.get() + 1));
    }

    let entry = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    entry.set_text("hi");

    assert_eq!(calls.get(), 2, "both handlers fan out");
}

// ---------------------------------------------------------------------
// on_text_end_editing — Entry::activate
// ---------------------------------------------------------------------

fn on_text_end_editing_fires_on_activate() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    let captured = Rc::new(Cell::new(String::new()));
    let c = captured.clone();
    el.on_text_end_editing(move |v| c.set(v));

    let entry = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    entry.set_text("done");
    entry.emit_activate();

    assert_eq!(captured.take(), "done");
}

fn on_change_and_on_input_coexist() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
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

    let entry = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    entry.set_text("x");
    entry.emit_activate();

    // GTK fires `changed` once per `set_text` call when going from
    // empty → non-empty (a single insert). Replacing existing
    // content fires it twice (delete + insert), unlike AppKit's
    // atomic `setStringValue:` — so the count is platform-specific.
    // Just assert it fired at least once.
    assert!(inputs.get() >= 1, "input should fire on text change");
    assert_eq!(changes.get(), 1, "change fires once on activate");
}

// ---------------------------------------------------------------------
// Element value getters (slider, popup, checkbox)
// ---------------------------------------------------------------------

fn slider_double_value_round_trips() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_slider(&tree).0;
    el.set_slider_min(0.0);
    el.set_slider_max(100.0);
    el.set_double_value(42.5);
    assert!((el.double_value() - 42.5).abs() < 1e-9);
}

fn popup_items_and_selection() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_pop_up_button(&tree).0;
    let items: Vec<String> =
        ["Alpha", "Beta", "Gamma"].into_iter().map(String::from).collect();
    el.set_popup_items(&items);
    el.set_popup_selection(2);
    assert_eq!(el.popup_selection(), 2);
}

fn checkbox_checked_round_trips() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    assert!(!el.checked());
    el.set_checked(true);
    assert!(el.checked());
}

fn main() {
    common::run_tests(&[
        // on_click
        ("on_click_fires_on_button", on_click_fires_on_button),
        ("on_click_on_label_is_no_op", on_click_on_label_is_no_op),
        (
            "on_click_on_checkbox_fires_on_toggle",
            on_click_on_checkbox_fires_on_toggle,
        ),
        // on_action
        ("on_action_fires_on_slider", on_action_fires_on_slider),
        ("on_action_fires_on_dropdown", on_action_fires_on_dropdown),
        ("on_action_on_view_is_no_op", on_action_on_view_is_no_op),
        // Text change
        (
            "on_text_change_fires_on_text_field",
            on_text_change_fires_on_text_field,
        ),
        ("on_text_change_on_button_is_no_op", on_text_change_on_button_is_no_op),
        ("multiple_on_text_change_fan_out", multiple_on_text_change_fan_out),
        // End editing
        (
            "on_text_end_editing_fires_on_activate",
            on_text_end_editing_fires_on_activate,
        ),
        ("on_change_and_on_input_coexist", on_change_and_on_input_coexist),
        // Value getters
        ("slider_double_value_round_trips", slider_double_value_round_trips),
        ("popup_items_and_selection", popup_items_and_selection),
        ("checkbox_checked_round_trips", checkbox_checked_round_trips),
        ]);
}
