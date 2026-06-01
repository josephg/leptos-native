//! Layout regression tests for `leptos_uikit`.
//!
//! Mirror of the cocoa side's `cocoa/leptos_cocoa/tests/layout.rs`,
//! but written at the `leptos_uikit` layer (no leptos_uikit needed) since
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
//!   cargo test -p leptos_uikit --target aarch64-apple-ios-sim
//!
//! Requires a booted simulator (`xcrun simctl boot ...`).


#[cfg(target_os = "ios")]
mod common;

#[cfg(target_os = "ios")]
mod ios {
    use super::common;

    use leptos_uikit::dom::{
        layout::{compute_layout, set_padding},
        UikitElem, UikitMakeView, UikitNodeExt,
    };
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSSize;
    use objc2_ui_kit::{UIButton, UILabel};

    /// Build a "content_root"–style UikitElem with a pinned size.
    fn make_root_with_size(width: f64, height: f64) -> UikitElem {
        let _mtm = common::test_mtm();

        let root = UikitElem::create_vstack();
        // Pin the root's size in its style so layout has something to
        // distribute against.
        root.with_style_mut(|s| {
            s.size = leptos_uikit::dom::layout::Size {
                width: leptos_uikit::dom::layout::Dimension::length(width as f32),
                height: leptos_uikit::dom::layout::Dimension::length(height as f32),
            };
        });

        root
    }

    /// REGRESSION: a `<vstack>` containing a single `<label>` must produce
    /// a non-zero height for the label after `compute_layout`. If the
    /// measure callback isn't invoked on the UILabel (e.g. because a stray
    /// placeholder turned it into a non-leaf in Taffy), the label's frame
    /// stays at 0×0 and the parent collapses.
    fn label_in_vstack_has_nonzero_height() {
        let _mtm = common::test_mtm();

        let root = make_root_with_size(320.0, 480.0);
        set_padding(root, 12.0);

        let label = UikitElem::create_label().0;
        label.set_title("Hello, iOS!");
        root.insert_node(label, None);

        compute_layout(
            root,
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
            "leptos_uikit UikitElem with tag=label didn't produce a UILabel"
        );
    }

    /// REGRESSION: buttons in an hstack must each get their natural
    /// intrinsic-content-sized frame after layout. Same shape as
    /// cocoa's `button_in_hstack_has_natural_size` test.
    fn buttons_in_hstack_have_natural_size() {
        let _mtm = common::test_mtm();

        let root = make_root_with_size(320.0, 200.0);

        let hstack = UikitElem::create_hstack();
        root.insert_node(hstack, None);

        let b1 = UikitElem::create_button().0;
        b1.set_title("OK");
        hstack.insert_node(b1, None);

        let b2 = UikitElem::create_button().0;
        b2.set_title("Cancel");
        hstack.insert_node(b2, None);

        compute_layout(
            root,
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
        let _mtm = common::test_mtm();

        let root = make_root_with_size(320.0, 480.0);
        set_padding(root, 12.0);

        let label = UikitElem::create_label().0;
        label.set_title("Count: 0");
        root.insert_node(label, None);

        let hstack = UikitElem::create_hstack();
        root.insert_node(hstack, None);
        for title in ["-1", "Reset", "+1"] {
            let b = UikitElem::create_button().0;
            b.set_title(title);
            hstack.insert_node(b, None);
        }

        compute_layout(
            root,
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

    pub fn run() {
        println!("leptos_uikit layout regression tests");
        common::run_tests(&[
            ("label_in_vstack_has_nonzero_height", label_in_vstack_has_nonzero_height),
            ("buttons_in_hstack_have_natural_size", buttons_in_hstack_have_natural_size),
            ("vstack_label_plus_hstack_has_full_height", vstack_label_plus_hstack_has_full_height),
        ]);
    }
}


#[cfg(target_os = "ios")]
fn main() {
    ios::run();
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("ios tests not run on non-ios platform");
}