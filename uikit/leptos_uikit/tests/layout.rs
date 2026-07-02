//! Builder-layer layout regression tests for `leptos_uikit`. Mirror of
//! `cocoa/leptos_cocoa/tests/layout.rs`, exercising the same Render→
//! Mountable→compute_layout pipeline through the iOS builders.
//!
//! Runs on the iOS simulator via the `.cargo/config.toml` runner:
//!
//!     cargo test --manifest-path uikit/leptos_uikit/Cargo.toml \
//!       --target aarch64-apple-ios-sim --test layout
//!
//! Requires a booted simulator.

#[cfg(target_os = "ios")]
mod common;

#[cfg(target_os = "ios")]
mod ios {
    use super::common;

    use leptos_uikit::ios::element::{button, hstack, label, vstack};
    use leptos_uikit::dom::{UikitMakeView, UikitNodeExt};
    use objc2::runtime::AnyObject;
    use objc2_ui_kit::{UIButton, UILabel};
    use reactive_graph::owner::Owner;
    use leptos_native::renderer::attrs::WithLayout;
    use leptos_native::renderer::view::{Mountable, Render};

    /// Spin up a fresh reactive owner + the spawner the iOS effect
    /// machinery needs.
    fn with_reactive_scope<F: FnOnce()>(body: F) {
        let _ = leptos_uikit::dom::spawner::init();
        let owner = Owner::new();
        owner.with(body);
    }

    /// Construct a content_root-style UikitElem with a Taffy tree, build
    /// `view`, mount it, run compute_layout against `size`, and call `f`
    /// with the content root.
    fn with_mounted_view<V, F>(view: V, size: (f64, f64), f: F)
    where
        V: Render<leptos_uikit::IosBackend>,
        V::State: Mountable<leptos_uikit::IosBackend>,
        F: FnOnce(&leptos_uikit::dom::UikitElem),
    {
        let _mtm = common::test_mtm();

        let root = leptos_uikit::dom::UikitElem::create_vstack();

        let mut state = view.build();
        state.mount(root, None);

        leptos_uikit::dom::layout::compute_layout(
            root,
            objc2_foundation::NSSize::new(size.0, size.1),
        );

        f(&root);
        drop(state);
    }

    fn walk(view: &objc2_ui_kit::UIView, f: &mut impl FnMut(&objc2_ui_kit::UIView)) {
        f(view);
        for sv in view.subviews().iter() {
            walk(&sv, f);
        }
    }

    /// REGRESSION: leaf controls (label, button) must have non-zero
    /// frames after compute_layout. Same shape as the cocoa equivalent —
    /// guards against the `Render for () = UnitState placeholder`
    /// regression that turned every leaf into a non-leaf in Taffy.
    fn leaf_controls_have_nonzero_intrinsic_size() {
        with_reactive_scope(|| {
            let view = vstack().padding(16.0).gap(12.0).child(
                label().text("Hello, iOS!"),
            );
            with_mounted_view(view, (320.0, 480.0), |root| {
                let mut found_label = false;
                walk(&*root.ui_view(), &mut |v| {
                    let any: &AnyObject = v.as_ref();
                    if let Some(lbl) = any.downcast_ref::<UILabel>() {
                        found_label = true;
                        let frame = lbl.frame();
                        assert!(
                            frame.size.height > 0.0,
                            "label height is zero: {:?}",
                            frame
                        );
                        assert!(
                            frame.size.width > 0.0,
                            "label width is zero: {:?}",
                            frame
                        );
                    }
                });
                assert!(found_label, "no UILabel in subview tree");
            });
        });
    }

    fn buttons_in_hstack_have_natural_size() {
        with_reactive_scope(|| {
            let view = hstack()
                .gap(8.0)
                .child(button().title("OK"))
                .child(button().title("Cancel"));
            with_mounted_view(view, (320.0, 100.0), |root| {
                let mut button_frames = Vec::new();
                walk(&*root.ui_view(), &mut |v| {
                    let any: &AnyObject = v.as_ref();
                    if any.downcast_ref::<UIButton>().is_some() {
                        button_frames.push(v.frame());
                    }
                });
                assert_eq!(
                    button_frames.len(),
                    2,
                    "expected 2 buttons in tree, got {}",
                    button_frames.len()
                );
                for (i, f) in button_frames.iter().enumerate() {
                    assert!(
                        f.size.height > 0.0,
                        "button[{i}] height {} is zero",
                        f.size.height
                    );
                    assert!(
                        f.size.width > 0.0,
                        "button[{i}] width {} is zero",
                        f.size.width
                    );
                }
            });
        });
    }

    fn vstack_label_plus_hstack_has_full_height() {
        with_reactive_scope(|| {
            let view = vstack()
                .padding(16.0)
                .gap(12.0)
                .child(label().text("Count: 0"))
                .child(
                    hstack()
                        .gap(8.0)
                        .child(button().title("-1"))
                        .child(button().title("Reset"))
                        .child(button().title("+1")),
                );
            with_mounted_view(view, (320.0, 480.0), |root| {
                let subs = root.ui_view().subviews();
                assert!(
                    subs.iter().next().is_some(),
                    "content_root has no subviews"
                );
                let outer = subs.iter().next().expect("subview");
                let frame = outer.frame();
                // padding(16)*2 = 32, label ~20, gap 12, button ~32 →
                // ~96. Allow slop; main thing is non-trivial.
                assert!(
                    frame.size.height >= 60.0,
                    "vstack height {} suspiciously small (expected >= ~60)",
                    frame.size.height
                );
                assert_eq!(
                    frame.size.width, 320.0,
                    "vstack should fill its parent width"
                );
            });
        });
    }

    pub fn run() {
        println!("leptos_uikit builder-layer layout regression tests");
        common::run_tests(&[
            (
                "leaf_controls_have_nonzero_intrinsic_size",
                leaf_controls_have_nonzero_intrinsic_size,
            ),
            (
                "buttons_in_hstack_have_natural_size",
                buttons_in_hstack_have_natural_size,
            ),
            (
                "vstack_label_plus_hstack_has_full_height",
                vstack_label_plus_hstack_has_full_height,
            ),
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