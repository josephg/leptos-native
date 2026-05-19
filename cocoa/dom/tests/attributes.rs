//! Typed attribute setter / remover tests for `Element`.
//!
//! Covers `set_string_attribute` / `set_bool_attribute` /
//! `remove_string_attribute` / `remove_bool_attribute` plus the
//! `&str`-keyed Rndr-trait surface (`set_attribute(&str, &str)`).

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{BoolAttr, Element, StringAttr};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSButton, NSControl, NSTextField};

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
    // Bool names should not parse as string.
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
    // String names should not parse as bool.
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
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_string_attribute(StringAttr::Title, "Hello");
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.title().to_string(), "Hello");
}

fn value_set_on_text_field() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_string_attribute(StringAttr::Value, "abc");
    let any: &AnyObject = el.ns_view().as_ref();
    let c = any.downcast_ref::<NSControl>().unwrap();
    assert_eq!(c.stringValue().to_string(), "abc");
}

fn placeholder_set_on_text_field() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_string_attribute(StringAttr::Placeholder, "Type here");
    let any: &AnyObject = el.ns_view().as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(
        f.placeholderString().map(|s| s.to_string()).unwrap_or_default(),
        "Type here"
    );
}

fn title_on_non_button_is_no_op() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_label(&tree).0;
    // Should not panic; should not affect the label's content.
    el.set_string_attribute(StringAttr::Title, "ignored");
    let any: &AnyObject = el.ns_view().as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    // Label was created with empty string content; setting Title
    // shouldn't have changed stringValue.
    assert_eq!(f.stringValue().to_string(), "");
}

// ---------------------------------------------------------------------
// Bool attributes — Enabled, Hidden, Checked
// ---------------------------------------------------------------------

fn enabled_toggles_nscontrol() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    let any: &AnyObject = el.ns_view().as_ref();
    let c = any.downcast_ref::<NSControl>().unwrap();
    assert!(c.isEnabled(), "buttons start enabled");

    el.set_bool_attribute(BoolAttr::Enabled, false);
    assert!(!c.isEnabled());

    el.set_bool_attribute(BoolAttr::Enabled, true);
    assert!(c.isEnabled());
}

fn enabled_on_non_control_is_no_op() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_container(&tree);
    // No NSControl underneath — should silently no-op.
    el.set_bool_attribute(BoolAttr::Enabled, false);
    // No assertion possible on the missing control, but the call
    // itself must not panic. (If we got here, we're good.)
}

fn hidden_toggles_view() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_container(&tree);
    let v = el.ns_view();
    assert!(!v.isHidden(), "views start visible");

    el.set_bool_attribute(BoolAttr::Hidden, true);
    assert!(v.isHidden());

    el.set_bool_attribute(BoolAttr::Hidden, false);
    assert!(!v.isHidden());
}

fn checked_toggles_button_state() {
    use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn};
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.state(), NSControlStateValueOff, "starts off");

    el.set_bool_attribute(BoolAttr::Checked, true);
    assert_eq!(b.state(), NSControlStateValueOn);

    el.set_bool_attribute(BoolAttr::Checked, false);
    assert_eq!(b.state(), NSControlStateValueOff);
}

fn checked_on_non_button_is_no_op() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_bool_attribute(BoolAttr::Checked, true);
    // No state to read on a text field — calling without panic is
    // the assertion.
}

// ---------------------------------------------------------------------
// Removal
// ---------------------------------------------------------------------

fn remove_title_clears_button() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_string_attribute(StringAttr::Title, "Hi");
    el.remove_string_attribute(StringAttr::Title);
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.title().to_string(), "");
}

fn remove_placeholder_clears_text_field() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_text_field(&tree).0;
    el.set_string_attribute(StringAttr::Placeholder, "X");
    el.remove_string_attribute(StringAttr::Placeholder);
    let any: &AnyObject = el.ns_view().as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert!(
        f.placeholderString().is_none()
            || f.placeholderString().map(|s| s.length()).unwrap_or(0) == 0
    );
}

fn remove_hidden_makes_visible() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_container(&tree);
    el.set_bool_attribute(BoolAttr::Hidden, true);
    assert!(el.ns_view().isHidden());

    el.remove_bool_attribute(BoolAttr::Hidden);
    assert!(!el.ns_view().isHidden());
}

fn remove_enabled_resets_to_enabled() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_bool_attribute(BoolAttr::Enabled, false);
    el.remove_bool_attribute(BoolAttr::Enabled);
    let any: &AnyObject = el.ns_view().as_ref();
    let c = any.downcast_ref::<NSControl>().unwrap();
    assert!(c.isEnabled(), "remove(Enabled) should reset to NSControl default (true)");
}

fn remove_checked_resets_to_off() {
    use objc2_app_kit::NSControlStateValueOff;
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_checkbox(&tree).0;
    el.set_bool_attribute(BoolAttr::Checked, true);
    el.remove_bool_attribute(BoolAttr::Checked);
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.state(), NSControlStateValueOff);
}

// ---------------------------------------------------------------------
// `&str`-keyed Rndr-trait surface
// ---------------------------------------------------------------------

fn rndr_set_attribute_routes_string_variants() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_attribute("title", "via_str");
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.title().to_string(), "via_str");
}

fn rndr_set_attribute_skips_bool_attrs() {
    // The string entry point deliberately does NOT route bool attrs
    // through the bool setter — builders use the typed bool setter.
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    let any: &AnyObject = el.ns_view().as_ref();
    let c = any.downcast_ref::<NSControl>().unwrap();
    assert!(c.isEnabled());

    el.set_attribute("enabled", "false");
    // Should still be enabled — the string setter ignores bool names.
    assert!(c.isEnabled());
}

fn rndr_set_attribute_unknown_is_no_op() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_attribute("totally_unknown", "x");
    // No panic; no observable change.
}

fn rndr_remove_attribute_routes_to_typed() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_string_attribute(StringAttr::Title, "Will be cleared");
    el.remove_attribute("title");
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.title().to_string(), "");
}

fn rndr_remove_attribute_routes_bool() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_container(&tree);
    el.set_bool_attribute(BoolAttr::Hidden, true);
    el.remove_attribute("hidden");
    assert!(!el.ns_view().isHidden());
}

// ---------------------------------------------------------------------
// Same-value diff (idempotence)
// ---------------------------------------------------------------------

fn set_string_same_value_is_idempotent() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_string_attribute(StringAttr::Title, "X");
    // Setting the same value twice doesn't change observable state.
    // (We rely on the diff guard to skip the AppKit setter — the
    // observable assertion is just that the value stays "X".)
    el.set_string_attribute(StringAttr::Title, "X");
    el.set_string_attribute(StringAttr::Title, "X");
    let any: &AnyObject = el.ns_view().as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.title().to_string(), "X");
}

fn set_bool_same_value_is_idempotent() {
    let _mtm = common::test_mtm();
    let tree = cocoa_dom::layout::new_tree();
    let el = Element::create_button(&tree).0;
    el.set_bool_attribute(BoolAttr::Enabled, false);
    el.set_bool_attribute(BoolAttr::Enabled, false);
    let any: &AnyObject = el.ns_view().as_ref();
    let c = any.downcast_ref::<NSControl>().unwrap();
    assert!(!c.isEnabled());
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
        ("enabled_toggles_nscontrol", enabled_toggles_nscontrol),
        (
            "enabled_on_non_control_is_no_op",
            enabled_on_non_control_is_no_op,
        ),
        ("hidden_toggles_view", hidden_toggles_view),
        ("checked_toggles_button_state", checked_toggles_button_state),
        ("checked_on_non_button_is_no_op", checked_on_non_button_is_no_op),
        // Removal
        ("remove_title_clears_button", remove_title_clears_button),
        (
            "remove_placeholder_clears_text_field",
            remove_placeholder_clears_text_field,
        ),
        ("remove_hidden_makes_visible", remove_hidden_makes_visible),
        (
            "remove_enabled_resets_to_enabled",
            remove_enabled_resets_to_enabled,
        ),
        ("remove_checked_resets_to_off", remove_checked_resets_to_off),
        // Rndr trait surface
        (
            "rndr_set_attribute_routes_string_variants",
            rndr_set_attribute_routes_string_variants,
        ),
        (
            "rndr_set_attribute_skips_bool_attrs",
            rndr_set_attribute_skips_bool_attrs,
        ),
        (
            "rndr_set_attribute_unknown_is_no_op",
            rndr_set_attribute_unknown_is_no_op,
        ),
        (
            "rndr_remove_attribute_routes_to_typed",
            rndr_remove_attribute_routes_to_typed,
        ),
        ("rndr_remove_attribute_routes_bool", rndr_remove_attribute_routes_bool),
        // Idempotence
        ("set_string_same_value_is_idempotent", set_string_same_value_is_idempotent),
        ("set_bool_same_value_is_idempotent", set_bool_same_value_is_idempotent),
    ]);
}
