//! Layout regression tests for `ios_dom`.
//!
//! Mirror of the cocoa side's `cocoa/leptos_cocoa/tests/layout.rs`,
//! but written at the `ios_dom` layer (no leptos_uikit needed) since
//! the iOS-side tachys-builder port is still in flight (TODO_ios.md).
//!
//! Bug under test: leaf controls (UIButton, UILabel) must end up with
//! non-zero frames after `compute_layout`. The cocoa side had a
//! regression where leaf controls' `Render::build` mounted a placeholder
//! UIView under the leaf, turning the leaf into a non-leaf in Taffy
//! and causing `intrinsicContentSize` measurement to be skipped. The
//! fix landed in `common/renderer/src/view/tuples.rs` (`Render for ()`
//! is now a no-op, so no stray placeholders).
//!
//! These tests verify the *measure pipeline itself* on iOS: with
//! correctly-shaped Taffy leaves (UIButton/UILabel directly under a
//! container), the measure callback returns sensible non-zero sizes.
//!
//! Runs on the iOS simulator via the cargo runner config in
//! `.cargo/config.toml`:
//!
//!   cargo test -p ios_dom --target aarch64-apple-ios-sim
//!
//! Requires a booted simulator (`xcrun simctl boot ...`).

#![cfg(target_os = "ios")]

mod common;

use ios_dom::{
    layout::{compute_layout, new_tree, set_as_root, set_padding},
    Element, StringAttr,
};
use objc2::runtime::AnyObject;
use objc2_foundation::NSSize;
use objc2_ui_kit::{UIButton, UILabel};

/// Build a fresh "content_root"–style Element registered in a
/// new Taffy tree. Mirrors what `SceneDelegate::scene:willConnectToSession`
/// does in production, but synchronously for tests.
fn make_root_with_size(width: f64, height: f64) -> (ios_dom::layout::TreeRef, Element) {
    let _mtm = common::test_mtm();
    let tree = new_tree();

    let root = Element::create_vstack(&tree);
    // Pin the root's size in its style so layout has something to
    // distribute against.
    root.as_node().with_style_mut(|s| {
        s.size = ios_dom::layout::Size {
            width: ios_dom::layout::Dimension::length(width as f32),
            height: ios_dom::layout::Dimension::length(height as f32),
        };
    });

    set_as_root(root.as_node(), &tree);
    (tree, root)
}

/// REGRESSION: a `<vstack>` containing a single `<label>` must produce
/// a non-zero height for the label after `compute_layout`. If the
/// measure callback isn't invoked on the UILabel (e.g. because a stray
/// placeholder turned it into a non-leaf in Taffy), the label's frame
/// stays at 0×0 and the parent collapses.
fn label_in_vstack_has_nonzero_height() {
    let mtm = common::test_mtm();

    let (tree, root) = make_root_with_size(320.0, 480.0);
    set_padding(root.as_node(), 12.0);

    let label = Element::create_label(&tree).0;
    label.set_string_attribute(StringAttr::Title, "Hello, iOS!");
    root.insert_node(label.as_node(), None);

    compute_layout(
        root.as_node(),
        NSSize::new(320.0, 480.0),
    );

    // UILabel's frame after layout must be non-zero. Find the label's
    // UIView and read the frame.
    let view = label.ui_view();
    let frame = view.frame();
    assert!(
        frame.size.height > 0.0,
        "label height is zero — measure callback didn't fire? frame={:?}",
        frame
    );
    assert!(
        frame.size.width > 0.0,
        "label width is zero. frame={:?}",
        frame
    );

    // Sanity: the underlying view IS a UILabel.
    let any: &AnyObject = view.as_ref();
    assert!(
        any.downcast_ref::<UILabel>().is_some(),
        "ios_dom Element with tag=label didn't produce a UILabel"
    );
}

/// REGRESSION: buttons in an hstack must each get their natural
/// intrinsic-content-sized frame after layout. Same shape as
/// cocoa's `button_in_hstack_has_natural_size` test.
fn buttons_in_hstack_have_natural_size() {
    let mtm = common::test_mtm();

    let (tree, root) = make_root_with_size(320.0, 200.0);

    let hstack = Element::create_hstack(&tree);
    root.insert_node(hstack.as_node(), None);

    let b1 = Element::create_button(&tree).0;
    b1.set_string_attribute(StringAttr::Title, "OK");
    hstack.insert_node(b1.as_node(), None);

    let b2 = Element::create_button(&tree).0;
    b2.set_string_attribute(StringAttr::Title, "Cancel");
    hstack.insert_node(b2.as_node(), None);

    compute_layout(
        root.as_node(),
        NSSize::new(320.0, 200.0),
    );

    for (name, btn) in &[("OK", &b1), ("Cancel", &b2)] {
        let view = btn.ui_view();
        let frame = view.frame();
        assert!(
            frame.size.height > 0.0,
            "{} button has zero height: {:?}",
            name,
            frame
        );
        assert!(
            frame.size.width > 0.0,
            "{} button has zero width: {:?}",
            name,
            frame
        );
        let any: &AnyObject = view.as_ref();
        assert!(
            any.downcast_ref::<UIButton>().is_some(),
            "{} button isn't a UIButton",
            name
        );
    }
}

/// Container test: a `<vstack>` containing a `<label>` and an
/// `<hstack>` of three buttons should produce a non-trivial composite
/// height. Mirrors the cocoa `vstack_label_plus_hstack_has_full_height`
/// test.
fn vstack_label_plus_hstack_has_full_height() {
    let mtm = common::test_mtm();

    let (tree, root) = make_root_with_size(320.0, 480.0);
    set_padding(root.as_node(), 12.0);

    let label = Element::create_label(&tree).0;
    label.set_string_attribute(StringAttr::Title, "Count: 0");
    root.insert_node(label.as_node(), None);

    let hstack = Element::create_hstack(&tree);
    root.insert_node(hstack.as_node(), None);
    for title in ["-1", "Reset", "+1"] {
        let b = Element::create_button(&tree).0;
        b.set_string_attribute(StringAttr::Title, title);
        hstack.insert_node(b.as_node(), None);
    }

    compute_layout(
        root.as_node(),
        NSSize::new(320.0, 480.0),
    );

    let root_frame = root.ui_view().frame();
    assert!(
        root_frame.size.height >= 40.0,
        "vstack composite height suspiciously small: {:?} \
         (expected >= ~40, label ~20 + buttons ~32)",
        root_frame
    );
    let label_frame = label.ui_view().frame();
    assert!(
        label_frame.size.height > 0.0,
        "inner label has zero height: {:?}",
        label_frame
    );
    let hstack_frame = hstack.ui_view().frame();
    assert!(
        hstack_frame.size.height > 0.0,
        "inner hstack has zero height: {:?}",
        hstack_frame
    );
}

fn main() {
    println!("ios_dom layout regression tests");
    common::run_tests(&[
        ("label_in_vstack_has_nonzero_height", label_in_vstack_has_nonzero_height),
        ("buttons_in_hstack_have_natural_size", buttons_in_hstack_have_natural_size),
        ("vstack_label_plus_hstack_has_full_height", vstack_label_plus_hstack_has_full_height),
    ]);
}
