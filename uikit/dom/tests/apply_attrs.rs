//! iOS smoke test for the `LayoutElement` / `UniversalElement` /
//! `LayoutNodeOps` impls — verifies the per-port glue that connects
//! `ios_dom::Element` / `ios_dom::Node` to the renderer-side
//! `apply_layout` / `apply_universal` machinery.
//!
//! **Coverage strategy:** the canonical, exhaustive coverage of
//! `apply_layout` / `apply_universal` (every LayoutAttrs field, every
//! Dim variant, every grid-placement enum, reactive vs. static, etc.)
//! lives in `cocoa/dom/tests/apply_attrs.rs`. Those tests exercise the
//! *generic* renderer code through cocoa's element type. Since the
//! generic functions don't know which port they're running against —
//! they only call methods on the trait — passing a single port is
//! enough to validate the install loop itself.
//!
//! What this file checks is the *port-specific glue*: that
//! `ios_dom::Element` actually routes through to the generic
//! `apply_layout` and that the trait impls behave as expected on a
//! UIView.
//!
//! Runs on the iOS simulator via:
//!
//!   cargo test -p ios_dom --target aarch64-apple-ios-sim --test apply_attrs


#[cfg(target_os = "ios")]
mod ios {
    mod common;

    use ios_dom::{layout, Element};
    use renderer::attrs::{LayoutAttrs, MaybeReactive, UniversalAttrs};


    fn style_of(el: &Element) -> renderer::Style {
        el.as_node().with_style(|s| s.clone())
    }

    fn padding_static_lands_in_padding_field() {
        let _mtm = common::test_mtm();
        let el = Element::create_vstack();

        let mut attrs = LayoutAttrs::default();
        attrs.padding = Some(MaybeReactive::Static(renderer::attrs::Edges::all(8.0)));
        let effects = layout::apply_layout(&el, attrs);
        assert!(effects.is_empty());
        assert_eq!(style_of(&el).padding.left, renderer::LengthPercentage::length(8.0));
    }

    fn flex_grow_static_lands_in_flex_grow() {
        let _mtm = common::test_mtm();
        let el = Element::create_vstack();

        let mut attrs = LayoutAttrs::default();
        attrs.flex_grow = Some(MaybeReactive::Static(2.0));
        let _ = layout::apply_layout(&el, attrs);
        assert_eq!(style_of(&el).flex_grow, 2.0);
    }

    fn empty_universal_attrs_no_panic() {
        let _mtm = common::test_mtm();
        let el = Element::create_vstack();
        let _ = layout::apply_universal(&el, UniversalAttrs::default());
    }

    fn alpha_static_sets_view_alpha() {
        let _mtm = common::test_mtm();
        let el = Element::create_vstack();

        let mut attrs = UniversalAttrs::default();
        attrs.alpha = Some(MaybeReactive::Static(0.5));
        let _ = layout::apply_universal(&el, attrs);

        let alpha = el.ui_view().alpha();
        assert!((alpha - 0.5).abs() < 1e-6, "got alpha={alpha}");
    }

    fn tool_tip_silently_dropped_on_ios() {
        // iOS has no hover-tooltip concept; the UniversalElement default
        // impl no-ops set_tool_tip. Verify it doesn't panic.
        let _mtm = common::test_mtm();
        let el = Element::create_vstack();

        let mut attrs = UniversalAttrs::default();
        attrs.tool_tip = Some(MaybeReactive::Static("ignored".to_string()));
        let _ = layout::apply_universal(&el, attrs);
    }

    fn main() {
        common::run_tests(&[
            ("padding_static_lands_in_padding_field", padding_static_lands_in_padding_field),
            ("flex_grow_static_lands_in_flex_grow", flex_grow_static_lands_in_flex_grow),
            ("empty_universal_attrs_no_panic", empty_universal_attrs_no_panic),
            ("alpha_static_sets_view_alpha", alpha_static_sets_view_alpha),
            ("tool_tip_silently_dropped_on_ios", tool_tip_silently_dropped_on_ios),
        ]);
    }
}


#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("ios tests not run on non-ios platform");
}