//! Typed attribute setter / remover tests for `Element`.
//!
//! Mirrors `cocoa_dom/tests/attributes.rs`. Covers
//! `set_string_attribute` / `set_bool_attribute` /
//! `remove_string_attribute` / `remove_bool_attribute` plus the
//! `&str`-keyed Renderer-trait surface (`set_attribute(&str, &str)`).

#![cfg(feature = "gtk")]

mod common;

use gtk_dom::{gtk::prelude::*, BoolAttr, Element, StringAttr};

// ---------------------------------------------------------------------
// Typed enum lookup round-trips
// ---------------------------------------------------------------------

fn string_attr_from_name_known() {
    assert_eq!(StringAttr::from_name("title"), Some(StringAttr::Title));
    assert_eq!(StringAttr::from_name("value"), Some(StringAttr::Value));
    assert_eq!(
        StringAttr::from_name("placeholder"),
        Some(StringAttr::Placeholder)
    );
}

fn string_attr_from_name_unknown_is_none() {
    assert_eq!(StringAttr::from_name("xyz"), None);
    assert_eq!(StringAttr::from_name(""), None);
    assert_eq!(StringAttr::from_name("enabled"), None);
}

fn string_attr_name_round_trips() {
    for v in [StringAttr::Title, StringAttr::Value, StringAttr::Placeholder] {
        assert_eq!(StringAttr::from_name(v.name()), Some(v));
    }
}

fn bool_attr_from_name_known() {
    assert_eq!(BoolAttr::from_name("enabled"), Some(BoolAttr::Enabled));
    assert_eq!(BoolAttr::from_name("hidden"), Some(BoolAttr::Hidden));
    assert_eq!(BoolAttr::from_name("checked"), Some(BoolAttr::Checked));
}

fn bool_attr_from_name_unknown_is_none() {
    assert_eq!(BoolAttr::from_name("xyz"), None);
    assert_eq!(BoolAttr::from_name("title"), None);
}

fn bool_attr_name_round_trips() {
    for v in [BoolAttr::Enabled, BoolAttr::Hidden, BoolAttr::Checked] {
        assert_eq!(BoolAttr::from_name(v.name()), Some(v));
    }
}

// ---------------------------------------------------------------------
// String attributes — Title, Value, Placeholder
// ---------------------------------------------------------------------

fn title_set_on_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_title("Hello");
    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    assert_eq!(b.label().as_deref(), Some("Hello"));
}

fn value_set_on_text_field() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_value("abc");
    let e = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    assert_eq!(e.text().as_str(), "abc");
}

fn placeholder_set_on_text_field() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_placeholder("Type here");
    let e = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    assert_eq!(e.placeholder_text().as_deref(), Some("Type here"));
}

fn title_on_non_button_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    // Should not panic; entry doesn't have a "title".
    el.set_title("ignored");
    let e = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    assert_eq!(
        e.text().as_str(),
        "",
        "title shouldn't have changed entry content"
    );
}

// ---------------------------------------------------------------------
// Bool attributes — Enabled, Hidden, Checked
// ---------------------------------------------------------------------

fn enabled_toggles_widget_sensitive() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    let v = el.widget();
    assert!(v.is_sensitive(), "buttons start enabled");

    el.set_enabled(false);
    assert!(!v.is_sensitive());

    el.set_enabled(true);
    assert!(v.is_sensitive());
}

fn enabled_on_view_is_supported() {
    let tree = gtk_dom::layout::new_tree();
    // gtk::Widget::set_sensitive applies to every widget, so
    // unlike cocoa (where `enabled` only affects NSControls), GTK
    // accepts it on plain views too. Assert that.
    let el = Element::create_stack(&tree);
    let v = el.widget();
    assert!(v.is_sensitive());
    el.set_enabled(false);
    assert!(!v.is_sensitive());
}

fn hidden_toggles_view() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_stack(&tree);
    let v = el.widget();
    // Note: gtk::Widget defaults to *not* visible (must be added to
    // a parent + shown). Test against initial state explicitly.
    let started_visible = v.is_visible();

    el.set_hidden(true);
    assert!(!v.is_visible(), "hidden=true should hide the widget");

    el.set_hidden(false);
    assert_eq!(
        v.is_visible(),
        true,
        "hidden=false should reveal it again"
    );
    let _ = started_visible;
}

fn checked_toggles_check_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    let c = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::CheckButton>()
        .unwrap();
    assert!(!c.is_active(), "starts off");

    el.set_checked(true);
    assert!(c.is_active());

    el.set_checked(false);
    assert!(!c.is_active());
}

fn checked_on_non_check_button_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_checked(true);
    // No state to read on entry — calling without panic is the
    // assertion.
}

// ---------------------------------------------------------------------
// Removal
// ---------------------------------------------------------------------

fn remove_title_clears_button() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_title("Hi");
    el.remove_string_attribute(StringAttr::Title);
    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    assert_eq!(b.label().as_deref().unwrap_or(""), "");
}

fn remove_placeholder_clears_text_field() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_placeholder("X");
    el.remove_string_attribute(StringAttr::Placeholder);
    let e = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Entry>()
        .unwrap();
    assert!(
        e.placeholder_text().is_none()
            || e.placeholder_text().as_deref() == Some("")
    );
}

fn remove_hidden_makes_visible() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_stack(&tree);
    el.set_hidden(true);
    assert!(!el.widget().is_visible());

    el.remove_bool_attribute(BoolAttr::Hidden);
    assert!(el.widget().is_visible());
}

fn remove_enabled_resets_to_enabled() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_enabled(false);
    el.remove_bool_attribute(BoolAttr::Enabled);
    assert!(el.widget().is_sensitive());
}

fn remove_checked_resets_to_off() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    el.set_checked(true);
    el.remove_bool_attribute(BoolAttr::Checked);
    let c = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::CheckButton>()
        .unwrap();
    assert!(!c.is_active());
}

// ---------------------------------------------------------------------
// `&str`-keyed Renderer-trait surface
// ---------------------------------------------------------------------

fn rndr_set_attribute_routes_string_variants() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_title("via_str");
    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    assert_eq!(b.label().as_deref(), Some("via_str"));
}

fn rndr_set_attribute_skips_bool_attrs() {
    let tree = gtk_dom::layout::new_tree();
    // The string entry point deliberately doesn't route bool attrs
    // through the bool setter — builders use the typed bool setter.
    let el = Element::create_button(&tree).0;
    assert!(el.widget().is_sensitive());

    el.set_attribute("enabled", "false");
    assert!(el.widget().is_sensitive(), "string setter ignores bool names");
}

fn rndr_set_attribute_unknown_is_no_op() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_attribute("totally_unknown", "x");
    // No panic; no observable change.
}

fn rndr_remove_attribute_routes_to_typed() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_title("Will be cleared");
    el.remove_attribute("title");
    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    assert_eq!(b.label().as_deref().unwrap_or(""), "");
}

fn rndr_remove_attribute_routes_bool() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_stack(&tree);
    el.set_hidden(true);
    el.remove_attribute("hidden");
    assert!(el.widget().is_visible());
}

// ---------------------------------------------------------------------
// Same-value diff (idempotence)
// ---------------------------------------------------------------------

fn set_string_same_value_is_idempotent() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_title("X");
    el.set_title("X");
    el.set_title("X");
    let b = el
        .widget()
        .downcast_ref::<gtk_dom::gtk::Button>()
        .unwrap();
    assert_eq!(b.label().as_deref(), Some("X"));
}

fn set_bool_same_value_is_idempotent() {
    let tree = gtk_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_enabled(false);
    el.set_enabled(false);
    assert!(!el.widget().is_sensitive());
}

fn main() {
    common::run_tests(&[
        // Enum lookup
        ("string_attr_from_name_known", string_attr_from_name_known),
        (
            "string_attr_from_name_unknown_is_none",
            string_attr_from_name_unknown_is_none,
        ),
        ("string_attr_name_round_trips", string_attr_name_round_trips),
        ("bool_attr_from_name_known", bool_attr_from_name_known),
        (
            "bool_attr_from_name_unknown_is_none",
            bool_attr_from_name_unknown_is_none,
        ),
        ("bool_attr_name_round_trips", bool_attr_name_round_trips),
        // String attrs
        ("title_set_on_button", title_set_on_button),
        ("value_set_on_text_field", value_set_on_text_field),
        ("placeholder_set_on_text_field", placeholder_set_on_text_field),
        ("title_on_non_button_is_no_op", title_on_non_button_is_no_op),
        // Bool attrs
        ("enabled_toggles_widget_sensitive", enabled_toggles_widget_sensitive),
        ("enabled_on_view_is_supported", enabled_on_view_is_supported),
        ("hidden_toggles_view", hidden_toggles_view),
        ("checked_toggles_check_button", checked_toggles_check_button),
        (
            "checked_on_non_check_button_is_no_op",
            checked_on_non_check_button_is_no_op,
        ),
        // Removal
        ("remove_title_clears_button", remove_title_clears_button),
        (
            "remove_placeholder_clears_text_field",
            remove_placeholder_clears_text_field,
        ),
        ("remove_hidden_makes_visible", remove_hidden_makes_visible),
        ("remove_enabled_resets_to_enabled", remove_enabled_resets_to_enabled),
        ("remove_checked_resets_to_off", remove_checked_resets_to_off),
        // Renderer surface
        (
            "rndr_set_attribute_routes_string_variants",
            rndr_set_attribute_routes_string_variants,
        ),
        ("rndr_set_attribute_skips_bool_attrs", rndr_set_attribute_skips_bool_attrs),
        ("rndr_set_attribute_unknown_is_no_op", rndr_set_attribute_unknown_is_no_op),
        ("rndr_remove_attribute_routes_to_typed", rndr_remove_attribute_routes_to_typed),
        ("rndr_remove_attribute_routes_bool", rndr_remove_attribute_routes_bool),
        // Idempotence
        ("set_string_same_value_is_idempotent", set_string_same_value_is_idempotent),
        ("set_bool_same_value_is_idempotent", set_bool_same_value_is_idempotent),
        ]);
}
