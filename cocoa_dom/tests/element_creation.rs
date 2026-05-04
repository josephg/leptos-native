//! Smoke tests for `Element::create` per tag.
//!
//! Verifies each tag string maps to an NSView whose dynamic class
//! matches what we expect (NSButton, NSTextField, NSSecureTextField,
//! NSSlider, NSPopUpButton, ...).
//!
//! Uses the custom main-thread harness (see
//! `cocoa_dom/tests/common/mod.rs`) — AppKit construction needs the
//! actual main thread.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::{Element, NodeKind};
use objc2::{runtime::AnyObject, DowncastTarget};
use objc2_app_kit::{
    NSButton, NSPopUpButton, NSSecureTextField, NSSlider, NSTextField,
    NSView,
};

/// Returns true if `view` is an instance of (or subclass of) `T`.
fn is_kind_of<T: DowncastTarget>(view: &NSView) -> bool {
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>().is_some()
}

fn view_is_plain_nsview() {
    let _mtm = common::test_mtm();
    let el = Element::create("view");
    assert_eq!(el.as_node().kind(), NodeKind::Element);
    let v = el.ns_view();
    assert!(!is_kind_of::<NSButton>(v));
    assert!(!is_kind_of::<NSTextField>(v));
    assert!(!is_kind_of::<NSSlider>(v));
    assert!(!is_kind_of::<NSPopUpButton>(v));
}

fn button_is_nsbutton() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    assert_eq!(el.as_node().kind(), NodeKind::Element);
    assert!(is_kind_of::<NSButton>(el.ns_view()));
}

fn checkbox_is_nsbutton() {
    let _mtm = common::test_mtm();
    let el = Element::create("checkbox");
    assert!(is_kind_of::<NSButton>(el.ns_view()));
}

fn label_is_nstextfield_non_editable() {
    let _mtm = common::test_mtm();
    let el = Element::create("label");
    let v = el.ns_view();
    assert!(is_kind_of::<NSTextField>(v));

    let any: &AnyObject = v.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert!(!field.isEditable(), "label should be non-editable");
}

fn text_field_is_nstextfield_editable() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    let v = el.ns_view();
    assert!(is_kind_of::<NSTextField>(v));

    let any: &AnyObject = v.as_ref();
    let field = any.downcast_ref::<NSTextField>().unwrap();
    assert!(field.isEditable(), "text_field should be editable");
    assert!(
        !is_kind_of::<NSSecureTextField>(v),
        "plain text_field shouldn't be secure"
    );
}

fn secure_text_field_is_nssecuretextfield() {
    let _mtm = common::test_mtm();
    let el = Element::create("secure_text_field");
    let v = el.ns_view();
    assert!(
        is_kind_of::<NSSecureTextField>(v),
        "secure_text_field should be NSSecureTextField"
    );
    assert!(
        is_kind_of::<NSTextField>(v),
        "NSSecureTextField subclasses NSTextField"
    );
}

fn slider_is_nsslider_continuous() {
    let _mtm = common::test_mtm();
    let el = Element::create("slider");
    let v = el.ns_view();
    assert!(is_kind_of::<NSSlider>(v));

    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSSlider>().unwrap();
    assert!(
        s.isContinuous(),
        "slider should fire target/action on every drag step"
    );
}

fn pop_up_button_is_nspopupbutton_pull_up() {
    let _mtm = common::test_mtm();
    let el = Element::create("pop_up_button");
    let v = el.ns_view();
    assert!(is_kind_of::<NSPopUpButton>(v));

    let any: &AnyObject = v.as_ref();
    let p = any.downcast_ref::<NSPopUpButton>().unwrap();
    assert!(
        !p.pullsDown(),
        "default popup should be pull-up (NO pullsDown)"
    );
}

fn unknown_tag_falls_back_to_view() {
    let _mtm = common::test_mtm();
    let el = Element::create("totally_made_up_zzz");
    let v = el.ns_view();
    assert!(!is_kind_of::<NSButton>(v));
    assert!(!is_kind_of::<NSTextField>(v));
    assert_eq!(el.as_node().kind(), NodeKind::Element);
}

fn kind_is_always_element() {
    let _mtm = common::test_mtm();
    for tag in [
        "view",
        "button",
        "checkbox",
        "label",
        "text_field",
        "secure_text_field",
        "slider",
        "pop_up_button",
        "stack_view",
        "totally_unknown_xyz",
    ] {
        let el = Element::create(tag);
        assert_eq!(
            el.as_node().kind(),
            NodeKind::Element,
            "tag {:?} should produce NodeKind::Element",
            tag
        );
    }
}

fn main() {
    common::run_tests(&[
        ("view_is_plain_nsview", view_is_plain_nsview),
        ("button_is_nsbutton", button_is_nsbutton),
        ("checkbox_is_nsbutton", checkbox_is_nsbutton),
        (
            "label_is_nstextfield_non_editable",
            label_is_nstextfield_non_editable,
        ),
        (
            "text_field_is_nstextfield_editable",
            text_field_is_nstextfield_editable,
        ),
        (
            "secure_text_field_is_nssecuretextfield",
            secure_text_field_is_nssecuretextfield,
        ),
        ("slider_is_nsslider_continuous", slider_is_nsslider_continuous),
        (
            "pop_up_button_is_nspopupbutton_pull_up",
            pop_up_button_is_nspopupbutton_pull_up,
        ),
        ("unknown_tag_falls_back_to_view", unknown_tag_falls_back_to_view),
        ("kind_is_always_element", kind_is_always_element),
    ]);
}
