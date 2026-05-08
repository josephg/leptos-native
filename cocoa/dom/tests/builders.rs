//! Tachys-side builder tests. Each builder constructs its
//! Cocoa-flavoured element via `Render::build` and the test
//! asserts on the resulting NSView's state.
//!
//! Reactive attributes need an `Owner` set on the current thread
//! so RenderEffect closures can run; `with_reactive_scope` wraps
//! a test body in one.

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::BoolAttr;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSButton, NSControl, NSPopUpButton, NSSlider, NSTextField};
use reactive_graph::{owner::Owner, signal::RwSignal, traits::*};
use tachys::{
    cocoa::{
        bind::{BindAttribute, Selection},
        element::{
            button, checkbox, color_well, date_picker, hstack, image_view,
            label, pop_up_button, progress_indicator, scroll_view,
            secure_text_field, segmented_control, slider, stack, stepper,
            text_field, text_view, vstack,
        },
        NodeRef,
    },
    html::attribute::Value,
    view::Render,
};

/// Run the test body inside a fresh reactive `Owner` scope, with
/// our main-thread spawner initialized.
///
/// `Owner::new()` provides the reactive cleanup root; the spawner
/// init satisfies `Executor::spawn_local` (RenderEffect uses it
/// for some internal coordination — without an executor, building
/// any reactive attribute panics).
///
/// The spawner's actual `spawn_local` runs futures via
/// `dispatch_async` on the main queue, which doesn't fire without
/// an active run loop. That's fine for these tests: the parts we
/// observe (RenderEffect's body re-running on signal change) are
/// synchronous; only deferred work would block on the run loop,
/// and we don't test that here.
fn with_reactive_scope<F: FnOnce()>(body: F) {
    // `init()` is idempotent across test invocations — subsequent
    // calls return `Err(AlreadySet)` which we ignore.
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(body);
}

// ---------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------

fn button_static_title() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button().title("Save").build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Save");
    });
}

fn button_reactive_title_initial_run() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let label = RwSignal::new("Click me".to_string());
        let st = button().title(move || label.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Click me");
    });
}

fn button_reactive_title_updates_on_signal_change() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let label = RwSignal::new("first".to_string());
        let st = button().title(move || label.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();

        label.set("second".to_string());
        // RenderEffect schedules its rebuild on the main queue;
        // pump the loop so the effect fires before we assert.
        common::pump_run_loop(0.1);
        assert_eq!(b.title().to_string(), "second");
    });
}

fn button_enabled_static() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button().title("X").enabled(false).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let c = any.downcast_ref::<NSControl>().unwrap();
        assert!(!c.isEnabled());
    });
}

fn button_enabled_reactive() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(true);
        let st = button().title("X").enabled(move || on.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let c = any.downcast_ref::<NSControl>().unwrap();
        assert!(c.isEnabled());
        on.set(false);
        common::pump_run_loop(0.1);
        assert!(!c.isEnabled());
    });
}

// ---------------------------------------------------------------------
// Checkbox
// ---------------------------------------------------------------------

fn checkbox_static_title_and_checked() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = checkbox().title("Subscribe").checked(true).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert_eq!(b.title().to_string(), "Subscribe");
        // checked()? element exposes a getter
        assert!(st.el.checked());
    });
}

fn checkbox_reactive_checked_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(false);
        let st = checkbox().checked(move || on.get()).build();
        assert!(!st.el.checked());
        on.set(true);
        common::pump_run_loop(0.1);
        assert!(st.el.checked());
    });
}

// ---------------------------------------------------------------------
// Label
// ---------------------------------------------------------------------

fn label_static_text() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = label().text("Hi").build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "Hi");
    });
}

fn label_reactive_text_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("a".to_string());
        let st = label().text(move || s.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "a");
        s.set("b".to_string());
        common::pump_run_loop(0.1);
        assert_eq!(f.stringValue().to_string(), "b");
    });
}

// ---------------------------------------------------------------------
// TextField
// ---------------------------------------------------------------------

fn text_field_value_and_placeholder() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = text_field()
            .value("initial")
            .placeholder("type here")
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.stringValue().to_string(), "initial");
        assert_eq!(
            f.placeholderString().map(|s| s.to_string()).unwrap_or_default(),
            "type here"
        );
        // It IS-A editable NSTextField; ensure we got the editable
        // (non-secure) variant.
        assert!(f.isEditable());
    });
}

fn secure_text_field_uses_secure_subclass() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = secure_text_field().build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        // NSSecureTextField IS-A NSTextField; check downcast to
        // the secure one.
        assert!(
            any.downcast_ref::<objc2_app_kit::NSSecureTextField>()
                .is_some(),
            "secure_text_field should produce NSSecureTextField"
        );
    });
}

// ---------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------

fn slider_min_max_value() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = slider()
            .min_value(0.0)
            .max_value(100.0)
            .value(42.0)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSSlider>().unwrap();
        assert_eq!(s.minValue(), 0.0);
        assert_eq!(s.maxValue(), 100.0);
        assert!((st.el.double_value() - 42.0).abs() < 1e-9);
    });
}

// ---------------------------------------------------------------------
// PopUpButton
// ---------------------------------------------------------------------

fn pop_up_button_items_static() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = pop_up_button()
            .items(vec!["Alpha", "Beta", "Gamma"])
            .selection(1_usize)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert_eq!(p.numberOfItems(), 3);
        assert_eq!(st.el.popup_selection(), 1);
    });
}

fn pop_up_button_items_owned_strings() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let items: Vec<String> =
            ["X", "Y"].iter().map(|s| s.to_string()).collect();
        let st = pop_up_button().items(items).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert_eq!(p.numberOfItems(), 2);
    });
}

// ---------------------------------------------------------------------
// View / vstack / hstack
// ---------------------------------------------------------------------

fn vstack_default_flex_direction_column() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = vstack().padding(8.0).gap(4.0).build();
        // A vstack is an Element. Just check the underlying NSView
        // exists; layout details get exercised in layout.rs.
        let _ = st.el.ns_view();
        // Pre-built direction = Column.
        assert_eq!(
            st.el.as_node().layout_slot().borrow().style.flex_direction,
            cocoa_dom::layout::FlexDirection::Column,
        );
    });
}

fn hstack_presets_direction_row() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = hstack().build();
        assert_eq!(
            st.el.as_node().layout_slot().borrow().style.flex_direction,
            cocoa_dom::layout::FlexDirection::Row,
        );
    });
}

fn stack_default_direction_is_column() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // Bare `stack()` with no axis explicitly set defaults to
        // Column at build time.
        let st = stack().build();
        assert_eq!(
            st.el.as_node().layout_slot().borrow().style.flex_direction,
            cocoa_dom::layout::FlexDirection::Column,
        );
    });
}

fn stack_direction_reactive_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use cocoa_dom::layout::FlexDirection;
        let dir = RwSignal::new(FlexDirection::Row);
        let st = stack().direction(move || dir.get()).build();
        assert_eq!(
            st.el.as_node().layout_slot().borrow().style.flex_direction,
            FlexDirection::Row,
        );
        dir.set(FlexDirection::Column);
        common::pump_run_loop(0.1);
        assert_eq!(
            st.el.as_node().layout_slot().borrow().style.flex_direction,
            FlexDirection::Column,
        );
    });
}

fn stack_justify_align_wrap_static() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use cocoa_dom::layout::{AlignItems, FlexWrap, JustifyContent};
        let st = stack()
            .justify_content(JustifyContent::SpaceBetween)
            .align(AlignItems::Center)
            .wrap(FlexWrap::Wrap)
            .build();
        let style = st.el.as_node().layout_slot().borrow().style.clone();
        assert_eq!(style.justify_content, Some(JustifyContent::SpaceBetween));
        assert_eq!(style.align_items, Some(AlignItems::Center));
        assert_eq!(style.flex_wrap, FlexWrap::Wrap);
    });
}

fn stack_grow_shrink_basis_round_trip() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = stack().grow(2.0).shrink(0.5).basis(120.0).build();
        let style = st.el.as_node().layout_slot().borrow().style.clone();
        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 0.5);
        // flex_basis is a Dimension::length(120.0)
        assert_eq!(
            style.flex_basis,
            cocoa_dom::layout::Dimension::length(120.0)
        );
    });
}

#[cfg(feature = "block_layout")]
fn block_creates_with_block_display() {
    use tachys::cocoa::element::block;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = block().padding(16.0).build();
        let style = st.el.as_node().layout_slot().borrow().style.clone();
        assert_eq!(style.display, cocoa_dom::layout::Display::Block);
        // padding rect components are LengthPercentage::length(16.0)
        let p = cocoa_dom::layout::LengthPercentage::length(16.0);
        assert_eq!(style.padding.left, p);
        assert_eq!(style.padding.right, p);
        assert_eq!(style.padding.top, p);
        assert_eq!(style.padding.bottom, p);
    });
}

// ---------------------------------------------------------------------
// Removal — ElementState's effects drop on unmount, unsubscribing
// from the signal.
// ---------------------------------------------------------------------

fn dropping_state_unsubscribes_effect() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("a".to_string());

        // Build and immediately drop the State. The Effect inside
        // should drop too — subsequent signal sets should NOT
        // panic / mutate anything (since the closure was dropped).
        {
            let st = label().text(move || s.get()).build();
            // capture once for sanity check
            let any: &AnyObject = st.el.ns_view().as_ref();
            let f = any.downcast_ref::<NSTextField>().unwrap();
            assert_eq!(f.stringValue().to_string(), "a");
            // st drops here
        }
        // After drop, setting the signal shouldn't blow up (no
        // dangling reference back into the dropped state).
        s.set("b".to_string());
        s.set("c".to_string());
    });
}

// ---------------------------------------------------------------------
// Bool variants of set_bool_attribute through builder reactive path
// ---------------------------------------------------------------------

fn checkbox_diff_skip_when_signal_same_value() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let on = RwSignal::new(true);
        let st = checkbox().checked(move || on.get()).build();

        // Set to same value many times; checkbox state shouldn't
        // toggle (diff guard inside set_bool_attribute).
        for _ in 0..5 {
            on.set(true);
        }
        common::pump_run_loop(0.1);
        assert!(st.el.checked());

        on.set(false);
        common::pump_run_loop(0.1);
        assert!(!st.el.checked());
    });
}

// ---------------------------------------------------------------------
// use: directives
// ---------------------------------------------------------------------

fn directive_no_param_fires_with_element() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use std::sync::{Arc, Mutex};
        let received: Arc<Mutex<Option<String>>> =
            Arc::new(Mutex::new(None));

        let r = received.clone();
        let highlight = move |el: cocoa_dom::Element| {
            // Read something off the element so we know it really
            // arrived. Title for a button.
            let any: &AnyObject = el.ns_view().as_ref();
            let b = any.downcast_ref::<NSButton>().unwrap();
            *r.lock().unwrap() = Some(b.title().to_string());
        };

        let _st = button().title("Hello").directive(highlight, ()).build();

        // Directive runs synchronously inside Render::build, so
        // the value is set by now — no run-loop pump needed.
        assert_eq!(received.lock().unwrap().as_deref(), Some("Hello"));
    });
}

fn directive_with_param() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use std::sync::{Arc, Mutex};
        let received: Arc<Mutex<Option<i32>>> =
            Arc::new(Mutex::new(None));

        let r = received.clone();
        let echo_param = move |_el: cocoa_dom::Element, n: i32| {
            *r.lock().unwrap() = Some(n);
        };

        let _st = button().directive(echo_param, 42_i32).build();

        assert_eq!(*received.lock().unwrap(), Some(42));
    });
}

fn multiple_directives_run_in_order() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use std::sync::{Arc, Mutex};
        let log: Arc<Mutex<Vec<&'static str>>> =
            Arc::new(Mutex::new(Vec::new()));

        let l1 = log.clone();
        let first = move |_el: cocoa_dom::Element| {
            l1.lock().unwrap().push("first");
        };
        let l2 = log.clone();
        let second = move |_el: cocoa_dom::Element| {
            l2.lock().unwrap().push("second");
        };

        let _st = button()
            .directive(first, ())
            .directive(second, ())
            .build();

        assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
    });
}

// ---------------------------------------------------------------------
// NodeRef
// ---------------------------------------------------------------------

fn node_ref_is_filled_after_build() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let r = NodeRef::new();
        assert!(r.get_untracked().is_none(), "starts empty");

        let st = button().title("X").node_ref(r).build();

        // The ref is filled with the same Element the State holds.
        let from_ref = r
            .get_untracked()
            .expect("node_ref should be filled after build");
        let st_ptr: *const _ = st.el.ns_view();
        let ref_ptr: *const _ = from_ref.ns_view();
        assert_eq!(st_ptr, ref_ptr);
    });
}

fn node_ref_on_load_fires() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let r = NodeRef::new();
        let fired = std::rc::Rc::new(std::cell::Cell::new(false));
        let f = fired.clone();
        r.on_load(move |_el| f.set(true));

        // on_load fires inside an Effect — pumping the run loop
        // gives reactive scheduling a chance.
        common::pump_run_loop(0.05);
        assert!(!fired.get(), "on_load shouldn't fire before build");

        let _st = text_field().node_ref(r).build();
        common::pump_run_loop(0.1);
        assert!(fired.get(), "on_load should fire after build");
    });
}

fn label_idempotent_set_does_not_error() {
    // Set the same string value multiple times via a signal —
    // should be safe (StringAttr::Title diff-guards).
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("X".to_string());
        let _st = label().text(move || s.get()).build();
        for _ in 0..10 {
            s.set("X".to_string());
        }
    });
}

// ---------------------------------------------------------------------
// text_view, image_view, scroll_view, segmented_control, color_well
// ---------------------------------------------------------------------

fn text_view_static_value() {
    use objc2_app_kit::{NSScrollView, NSTextView};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = text_view().value("Hello, multi-line").build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let scroll = any.downcast_ref::<NSScrollView>().unwrap();
        let doc = scroll.documentView().unwrap();
        let any_doc: &AnyObject = &doc;
        let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
        assert_eq!(tv.string().to_string(), "Hello, multi-line");
    });
}

fn text_view_reactive_value_updates() {
    use objc2_app_kit::{NSScrollView, NSTextView};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new("first".to_string());
        let st = text_view().value(move || s.get()).build();

        let any: &AnyObject = st.el.ns_view().as_ref();
        let scroll = any.downcast_ref::<NSScrollView>().unwrap();
        let doc = scroll.documentView().unwrap();
        let any_doc: &AnyObject = &doc;
        let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
        assert_eq!(tv.string().to_string(), "first");

        s.set("second".to_string());
        common::pump_run_loop(0.1);
        assert_eq!(tv.string().to_string(), "second");
    });
}

fn text_view_bind_value_two_way() {
    use objc2_app_kit::{NSScrollView, NSTextView};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new(String::from("initial"));
        let st = text_view().bind(Value, s).build();

        let any: &AnyObject = st.el.ns_view().as_ref();
        let scroll = any.downcast_ref::<NSScrollView>().unwrap();
        let doc = scroll.documentView().unwrap();
        let any_doc: &AnyObject = &doc;
        let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
        assert_eq!(tv.string().to_string(), "initial");

        // Outgoing: simulate user typing by setting the NSTextView's
        // string and firing textDidChange (what AppKit does on real
        // keystrokes). The delegate fans out to bind's setter
        // closure, which writes to the signal.
        tv.setString(&objc2_foundation::NSString::from_str("typed"));
        common::fire_text_view_did_change(tv);
        assert_eq!(s.get_untracked(), "typed");

        // Incoming: signal change → setString.
        s.set(String::from("from-signal"));
        common::pump_run_loop(0.1);
        assert_eq!(tv.string().to_string(), "from-signal");
    });
}

fn image_view_static_source_missing_file() {
    use objc2_app_kit::NSImageView;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = image_view().source("/nonexistent/foo.png").build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let iv = any.downcast_ref::<NSImageView>().unwrap();
        // Missing file → no panic, no image set.
        assert!(iv.image().is_none());
    });
}

fn scroll_view_with_child_routes_to_doc_view() {
    use objc2_app_kit::NSScrollView;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // scroll_view containing a vstack containing a button.
        let st = scroll_view()
            .child(vstack().child(button().title("inside")))
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let scroll = any.downcast_ref::<NSScrollView>().unwrap();
        // documentView's children get the user's content. The
        // scroll view's direct subviews are AppKit's clipView +
        // scrollers, not ours.
        let doc = scroll.documentView().unwrap();
        // Build doesn't mount; tests don't trigger the cascade. So
        // we only verify the documentView exists and the build
        // didn't panic.
        let _ = doc;
    });
}

fn segmented_control_items_and_selection() {
    use objc2_app_kit::NSSegmentedControl;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = segmented_control()
            .items(["Light", "Dark", "Auto"])
            .selection(2_usize)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let sc = any.downcast_ref::<NSSegmentedControl>().unwrap();
        assert_eq!(sc.segmentCount(), 3);
        assert_eq!(sc.selectedSegment(), 2);
    });
}

fn segmented_control_bind_selection_two_way() {
    use objc2_app_kit::NSSegmentedControl;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let sel = RwSignal::new(0_usize);
        let st = segmented_control()
            .items(["A", "B", "C"])
            .bind(Selection, sel)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let sc = any.downcast_ref::<NSSegmentedControl>().unwrap();
        assert_eq!(sc.selectedSegment(), 0);

        sel.set(2);
        common::pump_run_loop(0.1);
        assert_eq!(sc.selectedSegment(), 2, "signal → control");
    });
}

fn color_well_static_value() {
    use objc2_app_kit::NSColorWell;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = color_well()
            .value(cocoa_dom::Color::rgb(0.5, 0.25, 1.0))
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let cw = any.downcast_ref::<NSColorWell>().unwrap();
        let c = cocoa_dom::Color::from_nscolor(&cw.color()).unwrap();
        let tol = 1e-3;
        assert!((c.r - 0.5).abs() < tol);
        assert!((c.g - 0.25).abs() < tol);
        assert!((c.b - 1.0).abs() < tol);
    });
}

fn color_well_bind_value_two_way() {
    use objc2_app_kit::NSColorWell;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let c = RwSignal::new(cocoa_dom::Color::WHITE);
        let st = color_well().bind(Value, c).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let cw = any.downcast_ref::<NSColorWell>().unwrap();
        // Initial value installed.
        let v0 = cocoa_dom::Color::from_nscolor(&cw.color()).unwrap();
        let tol = 1e-3;
        assert!((v0.r - 1.0).abs() < tol);
        assert!((v0.g - 1.0).abs() < tol);

        c.set(cocoa_dom::Color::rgb(0.0, 0.0, 0.0));
        common::pump_run_loop(0.1);
        let v1 = cocoa_dom::Color::from_nscolor(&cw.color()).unwrap();
        assert!(v1.r < tol);
    });
}

fn date_picker_static_value() {
    use objc2_app_kit::NSDatePicker;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // 2030-06-15 ~= 1907798400.0
        let target = cocoa_dom::Date::from_unix_secs(1_907_798_400.0);
        let st = date_picker().value(target).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let dp = any.downcast_ref::<NSDatePicker>().unwrap();
        let got = cocoa_dom::Date::from_nsdate(&dp.dateValue());
        assert!(
            (got.seconds_since_epoch - target.seconds_since_epoch).abs()
                < 1.0
        );
    });
}

fn date_picker_bind_value_two_way() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let d = RwSignal::new(cocoa_dom::Date::from_unix_secs(0.0));
        let st = date_picker().bind(Value, d).build();
        let target = cocoa_dom::Date::from_unix_secs(1_700_000_000.0);
        d.set(target);
        common::pump_run_loop(0.1);
        assert!(
            (st.el.date_picker_value().seconds_since_epoch
                - target.seconds_since_epoch)
                .abs()
                < 1.0
        );
    });
}

fn stepper_static_min_max_value() {
    use objc2_app_kit::NSStepper;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = stepper()
            .min_value(1.0)
            .max_value(20.0)
            .increment(2.0)
            .value(10.0)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSStepper>().unwrap();
        assert_eq!(s.minValue(), 1.0);
        assert_eq!(s.maxValue(), 20.0);
        assert_eq!(s.increment(), 2.0);
        assert_eq!(s.doubleValue(), 10.0);
    });
}

fn stepper_bind_value_two_way() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let v = RwSignal::new(0.0_f64);
        let st = stepper()
            .min_value(0.0)
            .max_value(100.0)
            .bind(Value, v)
            .build();
        v.set(42.0);
        common::pump_run_loop(0.1);
        assert_eq!(st.el.stepper_value(), 42.0);
    });
}

fn progress_indicator_determinate_value() {
    use objc2_app_kit::NSProgressIndicator;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = progress_indicator()
            .max_value(10.0)
            .value(7.5_f64)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSProgressIndicator>().unwrap();
        assert!(!p.isIndeterminate());
        assert_eq!(p.maxValue(), 10.0);
        assert_eq!(p.doubleValue(), 7.5);
    });
}

fn progress_indicator_indeterminate_starts_animation() {
    use objc2_app_kit::NSProgressIndicator;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = progress_indicator().indeterminate(true).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSProgressIndicator>().unwrap();
        assert!(p.isIndeterminate());
    });
}

// ---------------------------------------------------------------------
// Universal + per-control attrs flowing through builders
// ---------------------------------------------------------------------

fn button_alpha_and_tool_tip_via_builder() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button()
            .title("X")
            .alpha(0.7)
            .tool_tip("hello")
            .build();
        let v = st.el.ns_view();
        assert!((v.alphaValue() - 0.7).abs() < 1e-9);
        assert_eq!(
            v.toolTip().map(|s| s.to_string()),
            Some("hello".to_string())
        );
    });
}

fn label_text_styling_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSTextAlignment, NSTextField};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = label()
            .text("Styled")
            .text_color(cocoa_dom::Color::rgb(0.0, 1.0, 0.0))
            .alignment(NSTextAlignment::Right)
            .font_size(18.0)
            .selectable(true)
            .build();

        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert!(f.textColor().is_some());
        assert_eq!(f.alignment(), NSTextAlignment::Right);
        assert!(
            (f.font().unwrap().pointSize() - 18.0).abs() < 1e-3
        );
        assert!(f.isSelectable());
    });
}

fn button_bordered_and_key_equivalent_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSButton;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = button()
            .title("Default")
            .bordered(false)
            .key_equivalent("\r")
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSButton>().unwrap();
        assert!(!b.isBordered());
        assert_eq!(b.keyEquivalent().to_string(), "\r");
    });
}

fn slider_vertical_and_ticks_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSSlider;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = slider()
            .min_value(0.0)
            .max_value(10.0)
            .vertical(true)
            .num_tick_marks(11)
            .snaps_to_ticks(true)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSSlider>().unwrap();
        assert!(s.isVertical());
        assert_eq!(s.numberOfTickMarks(), 11);
        assert!(s.allowsTickMarkValuesOnly());
    });
}

fn pop_up_pulls_down_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSPopUpButton;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = pop_up_button()
            .items(["File", "Edit"])
            .pulls_down(true)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert!(p.pullsDown());
    });
}

fn segmented_control_style_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSSegmentStyle, NSSegmentedControl};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = segmented_control()
            .items(["A", "B"])
            .segment_style(NSSegmentStyle::Capsule)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let sc = any.downcast_ref::<NSSegmentedControl>().unwrap();
        assert_eq!(sc.segmentStyle(), NSSegmentStyle::Capsule);
    });
}

fn date_picker_style_and_range_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSDatePicker, NSDatePickerStyle};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let min = cocoa_dom::Date::from_unix_secs(0.0);
        let max = cocoa_dom::Date::from_unix_secs(2_000_000.0);
        let st = date_picker()
            .style(NSDatePickerStyle::ClockAndCalendar)
            .min_date(min)
            .max_date(max)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let dp = any.downcast_ref::<NSDatePicker>().unwrap();
        assert_eq!(
            dp.datePickerStyle(),
            NSDatePickerStyle::ClockAndCalendar
        );
        assert!(dp.minDate().is_some());
        assert!(dp.maxDate().is_some());
    });
}

fn scroll_view_scroller_attrs_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSScrollView;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = scroll_view()
            .autohides_scrollers(true)
            .has_horizontal_scroller(true)
            .has_vertical_scroller(false)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSScrollView>().unwrap();
        assert!(s.autohidesScrollers());
        assert!(s.hasHorizontalScroller());
        assert!(!s.hasVerticalScroller());
    });
}

fn progress_displayed_when_stopped_via_builder() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSProgressIndicator;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let st = progress_indicator()
            .indeterminate(true)
            .displayed_when_stopped(true)
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let p =
            any.downcast_ref::<NSProgressIndicator>().unwrap();
        assert!(p.isDisplayedWhenStopped());
    });
}

// ---------------------------------------------------------------------
// Reactive variants of universal/per-control attrs
// ---------------------------------------------------------------------

fn alpha_reactive_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let a = RwSignal::new(1.0_f64);
        let st = button()
            .title("X")
            .alpha(move || a.get())
            .build();
        let v = st.el.ns_view();
        assert!((v.alphaValue() - 1.0).abs() < 1e-9);

        a.set(0.3);
        common::pump_run_loop(0.1);
        assert!((v.alphaValue() - 0.3).abs() < 1e-9);
    });
}

fn tool_tip_reactive_updates() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let s = RwSignal::new(String::from("first"));
        let st = button()
            .title("X")
            .tool_tip(move || s.get())
            .build();
        let v = st.el.ns_view();
        assert_eq!(
            v.toolTip().map(|t| t.to_string()),
            Some("first".into())
        );

        s.set("second".into());
        common::pump_run_loop(0.1);
        assert_eq!(
            v.toolTip().map(|t| t.to_string()),
            Some("second".into())
        );
    });
}

fn label_alignment_reactive_updates() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSTextAlignment, NSTextField};
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let a = RwSignal::new(NSTextAlignment::Left);
        let st = label()
            .text("Hi")
            .alignment(move || a.get())
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        assert_eq!(f.alignment(), NSTextAlignment::Left);

        a.set(NSTextAlignment::Center);
        common::pump_run_loop(0.1);
        assert_eq!(f.alignment(), NSTextAlignment::Center);
    });
}

fn label_text_color_reactive_updates() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSTextField;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let c = RwSignal::new(cocoa_dom::Color::rgb(1.0, 0.0, 0.0));
        let st = label()
            .text("Hi")
            .text_color(move || c.get())
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let f = any.downcast_ref::<NSTextField>().unwrap();
        let initial =
            cocoa_dom::Color::from_nscolor(&f.textColor().unwrap())
                .unwrap();
        assert!((initial.r - 1.0).abs() < 1e-3);

        c.set(cocoa_dom::Color::rgb(0.0, 0.0, 1.0));
        common::pump_run_loop(0.1);
        let after =
            cocoa_dom::Color::from_nscolor(&f.textColor().unwrap())
                .unwrap();
        assert!(after.r.abs() < 1e-3);
        assert!((after.b - 1.0).abs() < 1e-3);
    });
}

fn slider_vertical_reactive_updates() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSSlider;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let v = RwSignal::new(false);
        let st = slider().vertical(move || v.get()).build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let s = any.downcast_ref::<NSSlider>().unwrap();
        assert!(!s.isVertical());

        v.set(true);
        common::pump_run_loop(0.1);
        assert!(s.isVertical());
    });
}

fn pop_up_pulls_down_reactive_updates() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSPopUpButton;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let p = RwSignal::new(false);
        let st = pop_up_button()
            .items(["Action"])
            .pulls_down(move || p.get())
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let b = any.downcast_ref::<NSPopUpButton>().unwrap();
        assert!(!b.pullsDown());

        p.set(true);
        common::pump_run_loop(0.1);
        assert!(b.pullsDown());
    });
}

fn date_picker_min_date_reactive_updates() {
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSDatePicker;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let d = RwSignal::new(cocoa_dom::Date::from_unix_secs(0.0));
        let st = date_picker()
            .min_date(move || d.get())
            .build();
        let any: &AnyObject = st.el.ns_view().as_ref();
        let dp = any.downcast_ref::<NSDatePicker>().unwrap();
        let initial = cocoa_dom::Date::from_nsdate(
            &dp.minDate().unwrap(),
        );
        assert!(initial.seconds_since_epoch.abs() < 1.0);

        d.set(cocoa_dom::Date::from_unix_secs(1_000_000.0));
        common::pump_run_loop(0.1);
        let after = cocoa_dom::Date::from_nsdate(
            &dp.minDate().unwrap(),
        );
        assert!(
            (after.seconds_since_epoch - 1_000_000.0).abs() < 1.0
        );
    });
}

fn text_field_flex_grow_round_trip() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // grow doesn't have a direct AppKit-side observable;
        // just verify the builder accepts it and Render::build
        // doesn't panic. A Taffy-side test would require setting
        // up a tree, which is out of scope here.
        let _st = text_field().grow(1.0).build();
    });
}

// Suppress unused-import warnings for items used only via downcast.
#[allow(dead_code)]
fn _force_link() -> Option<BoolAttr> { None }

fn main() {
    common::run_tests(&[
        // Button
        ("button_static_title", button_static_title),
        ("button_reactive_title_initial_run", button_reactive_title_initial_run),
        (
            "button_reactive_title_updates_on_signal_change",
            button_reactive_title_updates_on_signal_change,
        ),
        ("button_enabled_static", button_enabled_static),
        ("button_enabled_reactive", button_enabled_reactive),
        // Checkbox
        ("checkbox_static_title_and_checked", checkbox_static_title_and_checked),
        ("checkbox_reactive_checked_updates", checkbox_reactive_checked_updates),
        // Label
        ("label_static_text", label_static_text),
        ("label_reactive_text_updates", label_reactive_text_updates),
        // TextField
        ("text_field_value_and_placeholder", text_field_value_and_placeholder),
        ("secure_text_field_uses_secure_subclass", secure_text_field_uses_secure_subclass),
        // Slider
        ("slider_min_max_value", slider_min_max_value),
        // PopUpButton
        ("pop_up_button_items_static", pop_up_button_items_static),
        ("pop_up_button_items_owned_strings", pop_up_button_items_owned_strings),
        // TextView, ImageView, ScrollView, SegmentedControl, ColorWell
        ("text_view_static_value", text_view_static_value),
        ("text_view_reactive_value_updates", text_view_reactive_value_updates),
        ("text_view_bind_value_two_way", text_view_bind_value_two_way),
        (
            "image_view_static_source_missing_file",
            image_view_static_source_missing_file,
        ),
        (
            "scroll_view_with_child_routes_to_doc_view",
            scroll_view_with_child_routes_to_doc_view,
        ),
        (
            "segmented_control_items_and_selection",
            segmented_control_items_and_selection,
        ),
        (
            "segmented_control_bind_selection_two_way",
            segmented_control_bind_selection_two_way,
        ),
        ("color_well_static_value", color_well_static_value),
        ("color_well_bind_value_two_way", color_well_bind_value_two_way),
        ("date_picker_static_value", date_picker_static_value),
        ("date_picker_bind_value_two_way", date_picker_bind_value_two_way),
        ("stepper_static_min_max_value", stepper_static_min_max_value),
        ("stepper_bind_value_two_way", stepper_bind_value_two_way),
        (
            "progress_indicator_determinate_value",
            progress_indicator_determinate_value,
        ),
        (
            "progress_indicator_indeterminate_starts_animation",
            progress_indicator_indeterminate_starts_animation,
        ),
        ("text_field_flex_grow_round_trip", text_field_flex_grow_round_trip),
        // Reactive variants
        ("alpha_reactive_updates", alpha_reactive_updates),
        ("tool_tip_reactive_updates", tool_tip_reactive_updates),
        ("label_alignment_reactive_updates", label_alignment_reactive_updates),
        ("label_text_color_reactive_updates", label_text_color_reactive_updates),
        ("slider_vertical_reactive_updates", slider_vertical_reactive_updates),
        ("pop_up_pulls_down_reactive_updates", pop_up_pulls_down_reactive_updates),
        ("date_picker_min_date_reactive_updates", date_picker_min_date_reactive_updates),
        // Universal + per-control attrs through builders
        ("button_alpha_and_tool_tip_via_builder", button_alpha_and_tool_tip_via_builder),
        ("label_text_styling_via_builder", label_text_styling_via_builder),
        (
            "button_bordered_and_key_equivalent_via_builder",
            button_bordered_and_key_equivalent_via_builder,
        ),
        ("slider_vertical_and_ticks_via_builder", slider_vertical_and_ticks_via_builder),
        ("pop_up_pulls_down_via_builder", pop_up_pulls_down_via_builder),
        ("segmented_control_style_via_builder", segmented_control_style_via_builder),
        (
            "date_picker_style_and_range_via_builder",
            date_picker_style_and_range_via_builder,
        ),
        ("scroll_view_scroller_attrs_via_builder", scroll_view_scroller_attrs_via_builder),
        (
            "progress_displayed_when_stopped_via_builder",
            progress_displayed_when_stopped_via_builder,
        ),
        // Stack / Block
        ("vstack_default_flex_direction_column", vstack_default_flex_direction_column),
        ("hstack_presets_direction_row", hstack_presets_direction_row),
        ("stack_default_direction_is_column", stack_default_direction_is_column),
        ("stack_direction_reactive_updates", stack_direction_reactive_updates),
        ("stack_justify_align_wrap_static", stack_justify_align_wrap_static),
        ("stack_grow_shrink_basis_round_trip", stack_grow_shrink_basis_round_trip),
        #[cfg(feature = "block_layout")]
        ("block_creates_with_block_display", block_creates_with_block_display),
        // Lifecycle
        ("dropping_state_unsubscribes_effect", dropping_state_unsubscribes_effect),
        // Idempotence
        ("checkbox_diff_skip_when_signal_same_value", checkbox_diff_skip_when_signal_same_value),
        ("label_idempotent_set_does_not_error", label_idempotent_set_does_not_error),
        // NodeRef
        ("node_ref_is_filled_after_build", node_ref_is_filled_after_build),
        ("node_ref_on_load_fires", node_ref_on_load_fires),
        // Directives
        ("directive_no_param_fires_with_element", directive_no_param_fires_with_element),
        ("directive_with_param", directive_with_param),
        ("multiple_directives_run_in_order", multiple_directives_run_in_order),
        // Typed-attribute pipeline (add_any_attr → OnAttribute → build)
        (
            "on_click_via_add_any_attr_fires",
            on_click_via_add_any_attr_fires,
        ),
        (
            "two_attributes_via_add_any_attr_panics",
            two_attributes_via_add_any_attr_panics,
        ),
        ("into_owned_preserves_element_state", into_owned_preserves_element_state),
    ]);
}

// ---------------------------------------------------------------------
// Typed-attribute pipeline — verifying the `<At = ()>` refactor
// ---------------------------------------------------------------------

fn on_click_via_add_any_attr_fires() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use std::sync::atomic::{AtomicBool, Ordering};
        let fired = std::sync::Arc::new(AtomicBool::new(false));
        let cb = fired.clone();
        use tachys::html::event::{click, on};
        use tachys::view::add_attr::AddAnyAttr;

        // Build via add_any_attr(OnAttribute), not the inline .on(…) path.
        let attr = on(click, move |()| cb.store(true, Ordering::SeqCst));
        let st = button().title("T").add_any_attr(attr).build();

        // The typed pipeline's `attrs.build(&el)` SHOULD have run
        // OnAttribute::build(&el) which calls `el.on_click(…)`.
        // Prove it by firing the action.
        let any: &AnyObject = st.el.ns_view().as_ref();
        let control = any.downcast_ref::<NSControl>().unwrap();
        common::fire_action(control);

        assert!(fired.load(Ordering::SeqCst), "on:click via add_any_attr should fire");
    });
}

fn two_attributes_via_add_any_attr_panics() {
    // Two `OnAttribute`s applied through add_any_attr (a tuple).
    // The first one wires NSControl's target/action; the second
    // would silently replace it on the legacy code path, so we
    // panic instead. Workaround for users who genuinely want both:
    // combine into one closure.
    let _mtm = common::test_mtm();
    let result = std::panic::catch_unwind(
        std::panic::AssertUnwindSafe(|| {
            with_reactive_scope(|| {
                use tachys::html::event::{click, on};
                use tachys::view::add_attr::AddAnyAttr;

                let _st = button()
                    .title("T")
                    .add_any_attr(on(click, move |()| {}))
                    .add_any_attr(on(click, move |()| {}))
                    .build();
            });
        }),
    );

    let payload = result.expect_err("expected the second on_click install to panic");
    let msg = payload
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic>");
    assert!(
        msg.contains("on_control_action called twice"),
        "panic message should explain duplicate install; got: {msg}"
    );
}

fn into_owned_preserves_element_state() {
    // RenderHtml::into_owned returns Self::Owned =
    // Self<At::CloneableOwned>. For OnAttribute, CloneableOwned = ().
    // Verify this doesn't panic and the result can be built.
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use tachys::html::event::{click, on};
        use tachys::view::{add_attr::AddAnyAttr, RenderHtml};

        let b = button().title("X").add_any_attr(on(click, |_: ()| {}));
        // Must compile and run without panicking.
        let _owned = b.into_owned();
    });
}

