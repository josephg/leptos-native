//! iOS `dom`-layer tests for the newly added controls / attributes:
//! `<tab_bar>`, `<blur_view>`, label `lines`, `font_weight`, drop
//! shadows, adaptive [`Color::dynamic`], programmatic scroll offset,
//! and the shared hidden-layout fix in `renderer::scene`.
//!
//! Runs on the iOS simulator via the runner config in
//! `.cargo/config.toml`:
//!
//!   cargo test -p leptos_uikit --target aarch64-apple-ios-sim \
//!       --test dom_new_features
//!
//! Requires a booted simulator.

#[cfg(target_os = "ios")]
mod common;

#[cfg(target_os = "ios")]
mod ios {
    use super::common;

    use leptos_uikit::dom::{
        event::{on_tab_select, tab_bar_delegate_store_size_for_test},
        layout,
        objc_enums::BlurStyle,
        Color, UikitElem, UikitMakeView, UikitNodeExt,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use objc2_ui_kit::{
        UILabel, UIScrollView, UITabBar, UITraitCollection,
        UIUserInterfaceStyle, UIVisualEffectView,
    };

    // -----------------------------------------------------------------
    // tab_bar
    // -----------------------------------------------------------------

    fn tab_bar_items_and_selection() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_tab_bar().0;
        el.set_tab_items(&[
            ("Home".into(), "house".into()),
            ("Search".into(), "magnifyingglass".into()),
            ("Me".into(), "person".into()),
        ]);

        // Selecting a valid index round-trips through the native bar.
        el.set_tab_selection(1);
        assert_eq!(el.tab_selected_index(), Some(1));
        el.set_tab_selection(2);
        assert_eq!(el.tab_selected_index(), Some(2));
        el.set_tab_selection(0);
        assert_eq!(el.tab_selected_index(), Some(0));
    }

    fn tab_bar_out_of_range_selection_is_noop() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_tab_bar().0;
        el.set_tab_items(&[("A".into(), "star".into())]);
        el.set_tab_selection(0);
        assert_eq!(el.tab_selected_index(), Some(0));
        // idx past the end must not change the current selection.
        el.set_tab_selection(99);
        assert_eq!(el.tab_selected_index(), Some(0));
    }

    fn tab_bar_delegate_installs_and_releases() {
        let _mtm = common::test_mtm();
        let baseline = tab_bar_delegate_store_size_for_test();

        // Wrap the whole lifecycle in one pool: UIKit autoreleases the
        // delegate somewhere during `setDelegate:`, so the final release
        // only lands on a pool drain (every run-loop tick in a live
        // app). Assert the count AFTER the pool closes.
        objc2::rc::autoreleasepool(|_| {
            let el = UikitElem::create_tab_bar().0;
            el.set_tab_items(&[("A".into(), "star".into())]);
            on_tab_select(el, |_idx| {});

            assert_eq!(tab_bar_delegate_store_size_for_test(), baseline + 1);
            let tb = el.try_downcast::<UITabBar>().unwrap();
            assert!(tb.delegate().is_some(), "delegate wired onto the bar");
            drop(tb);

            el.remove();
        });
        assert_eq!(
            tab_bar_delegate_store_size_for_test(),
            baseline,
            "tab-bar delegate released on teardown — no leak"
        );
    }

    // -----------------------------------------------------------------
    // blur_view
    // -----------------------------------------------------------------

    fn blur_view_is_visual_effect_view() {
        let _mtm = common::test_mtm();
        let (el, ev) = UikitElem::create_blur_view(BlurStyle::SYSTEM_MATERIAL);
        assert!(el.try_downcast::<UIVisualEffectView>().is_some());
        assert!(ev.effect().is_some(), "blur effect present at creation");
        // Underlay: it must not intercept touches meant for siblings.
        assert!(!ev.isUserInteractionEnabled());
    }

    fn blur_view_set_style_swaps_effect() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_blur_view(BlurStyle::LIGHT).0;
        el.set_blur_style(BlurStyle::DARK);
        let ev = el.try_downcast::<UIVisualEffectView>().unwrap();
        assert!(ev.effect().is_some());
    }

    // -----------------------------------------------------------------
    // label lines + font weight
    // -----------------------------------------------------------------

    fn label_lines_sets_number_of_lines() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_label().0;
        el.set_label_lines(0); // unlimited
        let label = el.try_downcast::<UILabel>().unwrap();
        assert_eq!(label.numberOfLines(), 0);
        el.set_label_lines(3);
        assert_eq!(label.numberOfLines(), 3);
    }

    fn font_weight_preserves_point_size_on_label() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_label().0;
        el.set_font_size(22.0);
        el.set_font_weight(700); // bold
        let label = el.try_downcast::<UILabel>().unwrap();
        let font = label.font().expect("label always has a font");
        let size = unsafe { font.pointSize() };
        assert!(
            (size - 22.0).abs() < 1e-6,
            "font weight change kept the point size, got {size}"
        );
    }

    fn font_weight_applies_to_text_field() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_text_field().0;
        el.set_font_size(18.0);
        // Must not panic and must leave a font installed at the size.
        el.set_font_weight(300);
        let field = el.try_downcast::<objc2_ui_kit::UITextField>().unwrap();
        let font = field.font().expect("field font set");
        let size = unsafe { font.pointSize() };
        assert!((size - 18.0).abs() < 1e-6, "got {size}");
    }

    // -----------------------------------------------------------------
    // scroll offset
    // -----------------------------------------------------------------

    fn scroll_offset_y_clamps_to_content() {
        let _mtm = common::test_mtm();
        let el = UikitElem::create_scroll_view().0;
        let sv = el.try_downcast::<UIScrollView>().unwrap();
        sv.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(100.0, 100.0)));

        // Content shorter than the viewport: max offset is 0, so any
        // positive request is clamped down.
        sv.setContentSize(NSSize::new(100.0, 40.0));
        el.set_scroll_offset_y(1000.0, false);
        assert!(
            sv.contentOffset().y.abs() < 1e-6,
            "clamped to 0 when content fits, got {}",
            sv.contentOffset().y
        );

        // Content taller than the viewport: clamp to (content - bounds).
        sv.setContentSize(NSSize::new(100.0, 500.0));
        el.set_scroll_offset_y(1000.0, false);
        assert!(
            (sv.contentOffset().y - 400.0).abs() < 1e-6,
            "clamped to max 400, got {}",
            sv.contentOffset().y
        );
        // A value inside range passes through untouched.
        el.set_scroll_offset_y(120.0, false);
        assert!((sv.contentOffset().y - 120.0).abs() < 1e-6);
        // Negative requests floor at 0.
        el.set_scroll_offset_y(-50.0, false);
        assert!(sv.contentOffset().y.abs() < 1e-6);
    }

    // -----------------------------------------------------------------
    // Color::dynamic
    // -----------------------------------------------------------------

    fn dynamic_color_fallback_returns_light() {
        // Non-Rgba inputs can't be baked into a [r,g,b,a] pair, so the
        // constructor degrades to the light color unchanged.
        let light = Color::SYSTEM_BLUE;
        let got = Color::dynamic(light, Color::rgb(0.0, 0.0, 0.0));
        assert_eq!(got, light);
    }

    fn dynamic_color_resolves_per_trait_collection() {
        let _mtm = common::test_mtm();
        let c = Color::dynamic(
            Color::rgb(1.0, 0.0, 0.0), // light: red
            Color::rgb(0.0, 0.0, 1.0), // dark: blue
        );
        let ui = c.to_uicolor();

        let light_tc = UITraitCollection::traitCollectionWithUserInterfaceStyle(
            UIUserInterfaceStyle::Light,
        );
        let dark_tc = UITraitCollection::traitCollectionWithUserInterfaceStyle(
            UIUserInterfaceStyle::Dark,
        );

        let (lr, _lg, lb) = read_rgb(&ui.resolvedColorWithTraitCollection(&light_tc));
        let (dr, _dg, db) = read_rgb(&ui.resolvedColorWithTraitCollection(&dark_tc));

        assert!(lr > 0.9 && lb < 0.1, "light => red, got r={lr} b={lb}");
        assert!(dr < 0.1 && db > 0.9, "dark => blue, got r={dr} b={db}");
    }

    fn read_rgb(color: &objc2_ui_kit::UIColor) -> (f64, f64, f64) {
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;
        unsafe {
            color.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a);
        }
        (r, g, b)
    }

    // -----------------------------------------------------------------
    // drop shadow (Backend setters)
    // -----------------------------------------------------------------

    fn shadow_setters_apply_to_layer() {
        use leptos_native::renderer::apply_decoration;
        use leptos_native::renderer::attrs::{DecorationAttrs, MaybeReactive};

        let _mtm = common::test_mtm();
        let el = UikitElem::create_vstack();

        let mut attrs = DecorationAttrs::<Color>::default();
        attrs.shadow_color = Some(MaybeReactive::Static(Color::rgb(0.0, 0.0, 0.0)));
        attrs.shadow_radius = Some(MaybeReactive::Static(6.0));
        attrs.shadow_offset = Some(MaybeReactive::Static((0.0, 3.0)));
        attrs.shadow_opacity = Some(MaybeReactive::Static(0.5));
        let effects = apply_decoration(el, attrs);
        assert!(effects.is_empty(), "static attrs install no effects");

        let layer = el.ui_view().layer();
        assert!((layer.shadowOpacity() - 0.5).abs() < 1e-4);
        assert!((layer.shadowRadius() - 6.0).abs() < 1e-6);
        assert!(
            !layer.masksToBounds(),
            "a live shadow must disable masksToBounds so it isn't clipped"
        );
    }

    fn corner_radius_then_shadow_leaves_masking_off() {
        use leptos_native::renderer::apply_decoration;
        use leptos_native::renderer::attrs::{DecorationAttrs, MaybeReactive};

        let _mtm = common::test_mtm();
        let el = UikitElem::create_vstack();

        // Order that would naively re-enable clipping: radius first
        // (masksToBounds := true), then shadow. Final state must be off.
        let mut attrs = DecorationAttrs::<Color>::default();
        attrs.corner_radius = Some(MaybeReactive::Static(12.0));
        attrs.shadow_opacity = Some(MaybeReactive::Static(0.8));
        let _ = apply_decoration(el, attrs);
        assert!(!el.ui_view().layer().masksToBounds());
    }

    fn shadow_toggle_restores_corner_clipping() {
        use leptos_native::renderer::Backend;
        use leptos_uikit::__reexports::send_wrapper::SendWrapper;
        use leptos_uikit::IosBackend;

        let _mtm = common::test_mtm();
        let el = UikitElem::create_vstack();
        let view = SendWrapper::new(el.ui_view());

        // Rounded + clipping, no shadow yet.
        IosBackend::set_corner_radius(&view, 12.0);
        assert!(
            el.ui_view().layer().masksToBounds(),
            "corner radius clips when there is no shadow"
        );

        // Shadow on: clipping must yield so the shadow isn't cut off.
        IosBackend::set_shadow_opacity(&view, 0.8);
        assert!(
            !el.ui_view().layer().masksToBounds(),
            "a live shadow disables clipping"
        );

        // Shadow back off (the reactive toggle): clipping must return,
        // since the corner radius is still > 0. The old one-way disable
        // left this stuck off — this is the regression guard.
        IosBackend::set_shadow_opacity(&view, 0.0);
        assert!(
            el.ui_view().layer().masksToBounds(),
            "clipping restored when the shadow toggles back to 0"
        );
    }

    // -----------------------------------------------------------------
    // shared hidden-layout fix (renderer::scene)
    // -----------------------------------------------------------------

    fn hidden_container_with_visible_children_does_not_panic() {
        use leptos_uikit::dom::layout::{compute_layout, Display};

        let _mtm = common::test_mtm();
        let root = UikitElem::create_vstack();
        root.with_style_mut(|s| {
            s.size = layout::Size {
                width: layout::Dimension::length(200.0),
                height: layout::Dimension::length(200.0),
            };
        });

        // A `Display::None` container that still holds a visible leaf.
        // Taffy recurses into it with `PerformHiddenLayout`; without the
        // scene.rs short-circuit the leaf measure fires and panics.
        let hidden = UikitElem::create_vstack();
        hidden.with_style_mut(|s| s.display = Display::None);
        let hidden_label = UikitElem::create_label().0;
        layout::attach_child(hidden, hidden_label);

        let visible_label = UikitElem::create_label().0;

        layout::attach_child(root, hidden);
        layout::attach_child(root, visible_label);

        // The assertion is "returns without panicking".
        compute_layout(root, NSSize::new(200.0, 200.0));

        root.remove();
    }

    pub fn run() {
        common::run_tests(&[
            ("tab_bar_items_and_selection", tab_bar_items_and_selection),
            ("tab_bar_out_of_range_selection_is_noop", tab_bar_out_of_range_selection_is_noop),
            ("tab_bar_delegate_installs_and_releases", tab_bar_delegate_installs_and_releases),
            ("blur_view_is_visual_effect_view", blur_view_is_visual_effect_view),
            ("blur_view_set_style_swaps_effect", blur_view_set_style_swaps_effect),
            ("label_lines_sets_number_of_lines", label_lines_sets_number_of_lines),
            ("font_weight_preserves_point_size_on_label", font_weight_preserves_point_size_on_label),
            ("font_weight_applies_to_text_field", font_weight_applies_to_text_field),
            ("scroll_offset_y_clamps_to_content", scroll_offset_y_clamps_to_content),
            ("dynamic_color_fallback_returns_light", dynamic_color_fallback_returns_light),
            ("dynamic_color_resolves_per_trait_collection", dynamic_color_resolves_per_trait_collection),
            ("shadow_setters_apply_to_layer", shadow_setters_apply_to_layer),
            ("corner_radius_then_shadow_leaves_masking_off", corner_radius_then_shadow_leaves_masking_off),
            ("shadow_toggle_restores_corner_clipping", shadow_toggle_restores_corner_clipping),
            ("hidden_container_with_visible_children_does_not_panic", hidden_container_with_visible_children_does_not_panic),
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
