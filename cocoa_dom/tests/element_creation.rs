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
    NSButton, NSColorWell, NSDatePicker, NSImageView, NSPopUpButton,
    NSProgressIndicator, NSScrollView, NSSecureTextField,
    NSSegmentedControl, NSSlider, NSStepper, NSTextField, NSTextView,
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

fn text_view_is_scroll_view_with_textview_inside() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_view");
    let v = el.ns_view();
    assert!(
        is_kind_of::<NSScrollView>(v),
        "text_view should be backed by NSScrollView"
    );

    let any: &AnyObject = v.as_ref();
    let scroll = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = scroll
        .documentView()
        .expect("scroll view should have a document view");
    let any_doc: &AnyObject = &doc;
    assert!(
        any_doc.downcast_ref::<NSTextView>().is_some(),
        "document view should be NSTextView"
    );

    let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
    assert!(tv.isEditable(), "text_view should default to editable");
    assert!(!tv.isRichText(), "text_view should default to plain text");
}

fn text_view_value_round_trips() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_view");
    el.set_string_attribute(cocoa_dom::StringAttr::Value, "Hello, world");
    assert_eq!(el.text_view_value(), Some("Hello, world".to_string()));
}

fn text_view_set_editable_round_trips() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_view");
    el.set_text_view_editable(false);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let scroll = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = scroll.documentView().unwrap();
    let any_doc: &AnyObject = &doc;
    let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
    assert!(!tv.isEditable());

    el.set_text_view_editable(true);
    assert!(tv.isEditable());
}

fn scroll_view_is_nsscrollview_with_doc_view() {
    let _mtm = common::test_mtm();
    let el = Element::create("scroll_view");
    let v = el.ns_view();
    assert!(is_kind_of::<NSScrollView>(v));

    // documentView is the FlippedView we install at construction.
    // Children added via insert_node go inside it.
    let any: &AnyObject = v.as_ref();
    let scroll = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = scroll
        .documentView()
        .expect("scroll_view should have a document view");
    // documentView is an NSView (the FlippedView subclass; we don't
    // expose FlippedView publicly, so just check for NSView and
    // that isFlipped returns true).
    assert!(doc.isFlipped(), "documentView should be flipped");
}

fn scroll_view_subview_parent_routes_to_doc_view() {
    let _mtm = common::test_mtm();
    let el = Element::create("scroll_view");
    let parent = el.subview_parent();
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let scroll = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = scroll.documentView().unwrap();

    let parent_ptr: *const NSView = &*parent;
    let doc_ptr: *const NSView = &*doc;
    assert_eq!(
        parent_ptr, doc_ptr,
        "subview_parent should return the documentView for scroll_view"
    );
}

fn scroll_view_insert_routes_child_to_doc_view() {
    let _mtm = common::test_mtm();
    let scroll = Element::create("scroll_view");
    let inner = Element::create("button");
    scroll.insert_node(inner.as_node(), None);

    // The button should be a subview of documentView, NOT a direct
    // subview of NSScrollView.
    let v = scroll.ns_view();
    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = s.documentView().unwrap();

    let doc_subviews = doc.subviews();
    assert_eq!(
        doc_subviews.len(),
        1,
        "documentView should hold the inserted button"
    );

    let button_ptr: *const NSView = inner.ns_view();
    let first_doc_subview: *const NSView = &*doc_subviews.objectAtIndex(0);
    assert_eq!(button_ptr, first_doc_subview);
}

fn image_view_is_nsimageview() {
    let _mtm = common::test_mtm();
    let el = Element::create("image_view");
    assert!(is_kind_of::<NSImageView>(el.ns_view()));
}

fn image_view_set_path_with_missing_file_clears_image() {
    // NSImage::initWithContentsOfFile returns nil for invalid
    // paths; setImage(nil) is fine. Should not panic.
    let _mtm = common::test_mtm();
    let el = Element::create("image_view");
    el.set_image_view_path("/nonexistent/path/foo.png");

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let iv = any.downcast_ref::<NSImageView>().unwrap();
    assert!(iv.image().is_none(), "missing file → no image set");
}

fn image_view_empty_path_clears_image() {
    let _mtm = common::test_mtm();
    let el = Element::create("image_view");
    el.set_image_view_path("");

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let iv = any.downcast_ref::<NSImageView>().unwrap();
    assert!(iv.image().is_none());
}

fn date_picker_is_nsdatepicker() {
    let _mtm = common::test_mtm();
    let el = Element::create("date_picker");
    assert!(is_kind_of::<NSDatePicker>(el.ns_view()));
}

fn date_picker_value_round_trips() {
    let _mtm = common::test_mtm();
    let el = Element::create("date_picker");
    // 2030-01-01 UTC = 1893456000.0 Unix seconds.
    let target = cocoa_dom::Date::from_unix_secs(1_893_456_000.0);
    el.set_date_picker_value(target);
    let got = el.date_picker_value();
    assert!(
        (got.seconds_since_epoch - target.seconds_since_epoch).abs()
            < 1.0,
        "got {} expected {}",
        got.seconds_since_epoch,
        target.seconds_since_epoch
    );
}

fn stepper_is_nsstepper() {
    let _mtm = common::test_mtm();
    let el = Element::create("stepper");
    assert!(is_kind_of::<NSStepper>(el.ns_view()));
}

fn stepper_configure_and_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("stepper");
    el.configure_stepper(0.0, 10.0, 0.5);
    el.set_stepper_value(3.5);
    assert_eq!(el.stepper_value(), 3.5);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSStepper>().unwrap();
    assert_eq!(s.minValue(), 0.0);
    assert_eq!(s.maxValue(), 10.0);
    assert_eq!(s.increment(), 0.5);
}

fn progress_indicator_is_nsprogressindicator() {
    let _mtm = common::test_mtm();
    let el = Element::create("progress_indicator");
    assert!(is_kind_of::<NSProgressIndicator>(el.ns_view()));
}

fn progress_indicator_value_and_max() {
    let _mtm = common::test_mtm();
    let el = Element::create("progress_indicator");
    el.set_progress_max(10.0);
    el.set_progress_value(7.5);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let p = any.downcast_ref::<NSProgressIndicator>().unwrap();
    assert_eq!(p.maxValue(), 10.0);
    assert_eq!(p.doubleValue(), 7.5);
}

fn progress_indicator_indeterminate_toggles() {
    let _mtm = common::test_mtm();
    let el = Element::create("progress_indicator");
    el.set_progress_indeterminate(true);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let p = any.downcast_ref::<NSProgressIndicator>().unwrap();
    assert!(p.isIndeterminate());

    el.set_progress_indeterminate(false);
    assert!(!p.isIndeterminate());
}

fn color_well_is_nscolorwell() {
    let _mtm = common::test_mtm();
    let el = Element::create("color_well");
    assert!(is_kind_of::<NSColorWell>(el.ns_view()));
}

fn color_well_value_round_trips() {
    let _mtm = common::test_mtm();
    let el = Element::create("color_well");
    let red = cocoa_dom::Color::rgb(1.0, 0.0, 0.0);
    el.set_color_well_value(red);
    let got = el.color_well_value();
    // Components might shift slightly through colorspace conversion;
    // assert each within tight tolerance.
    let tol = 1e-3;
    assert!((got.r - 1.0).abs() < tol);
    assert!(got.g.abs() < tol);
    assert!(got.b.abs() < tol);
    assert!((got.a - 1.0).abs() < tol);
}

fn segmented_control_is_nssegmentedcontrol() {
    let _mtm = common::test_mtm();
    let el = Element::create("segmented_control");
    assert!(is_kind_of::<NSSegmentedControl>(el.ns_view()));
}

fn segmented_items_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("segmented_control");
    let items = ["Alpha", "Beta", "Gamma"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    el.set_segmented_items(&items);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let sc = any.downcast_ref::<NSSegmentedControl>().unwrap();
    assert_eq!(sc.segmentCount(), 3);
    assert_eq!(
        sc.labelForSegment(1).map(|s| s.to_string()),
        Some("Beta".to_string())
    );
}

fn segmented_selection_round_trips() {
    let _mtm = common::test_mtm();
    let el = Element::create("segmented_control");
    el.set_segmented_items(&["a".to_string(), "b".to_string()]);
    el.set_segmented_selection(1);
    assert_eq!(el.segmented_selection(), 1);
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
        "text_view",
        "image_view",
        "scroll_view",
        "slider",
        "pop_up_button",
        "segmented_control",
        "color_well",
        "date_picker",
        "stepper",
        "progress_indicator",
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

// ---------------------------------------------------------------------
// Universal attributes (alpha, tool_tip)
// ---------------------------------------------------------------------

fn alpha_round_trips_and_clamps() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    el.set_alpha(0.5);
    let v = el.ns_view();
    assert!((v.alphaValue() - 0.5).abs() < 1e-9);

    // Out-of-range values clamp to [0, 1].
    el.set_alpha(2.0);
    assert!((v.alphaValue() - 1.0).abs() < 1e-9);
    el.set_alpha(-0.5);
    assert!(v.alphaValue().abs() < 1e-9);
}

fn tool_tip_set_and_clear() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    el.set_tool_tip("Click me");
    let v = el.ns_view();
    assert_eq!(
        v.toolTip().map(|s| s.to_string()),
        Some("Click me".to_string())
    );
    // Empty string clears.
    el.set_tool_tip("");
    assert!(v.toolTip().is_none());
}

// ---------------------------------------------------------------------
// Text styling (text_color, alignment, font_size)
// ---------------------------------------------------------------------

fn text_color_on_text_field() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    let red = cocoa_dom::Color::rgb(1.0, 0.0, 0.0);
    el.set_text_color(red);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    let got = f.textColor().expect("textColor should be set");
    let c = cocoa_dom::Color::from_nscolor(&got).unwrap();
    let tol = 1e-3;
    assert!((c.r - 1.0).abs() < tol);
    assert!(c.g.abs() < tol);
    assert!(c.b.abs() < tol);
}

fn text_color_on_label() {
    let _mtm = common::test_mtm();
    let el = Element::create("label");
    el.set_text_color(cocoa_dom::Color::rgb(0.0, 0.5, 0.0));
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert!(f.textColor().is_some());
}

fn alignment_on_text_field() {
    use objc2_app_kit::NSTextAlignment;
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    el.set_text_alignment(NSTextAlignment::Center);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert_eq!(f.alignment(), NSTextAlignment::Center);
}

fn font_size_on_label() {
    let _mtm = common::test_mtm();
    let el = Element::create("label");
    el.set_font_size(20.0);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    let font = f.font().expect("font should be set");
    assert!((font.pointSize() - 20.0).abs() < 1e-3);
}

fn font_size_on_text_view() {
    use objc2_app_kit::{NSScrollView, NSTextView};
    let _mtm = common::test_mtm();
    let el = Element::create("text_view");
    el.set_font_size(16.0);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let scroll = any.downcast_ref::<NSScrollView>().unwrap();
    let doc = scroll.documentView().unwrap();
    let any_doc: &AnyObject = &doc;
    let tv = any_doc.downcast_ref::<NSTextView>().unwrap();
    let font = tv.font().expect("font should be set");
    assert!((font.pointSize() - 16.0).abs() < 1e-3);
}

// ---------------------------------------------------------------------
// Per-control statics
// ---------------------------------------------------------------------

fn button_bordered_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    el.set_button_bordered(false);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert!(!b.isBordered());
    el.set_button_bordered(true);
    assert!(b.isBordered());
}

fn button_key_equivalent_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("button");
    el.set_key_equivalent("\r");
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let b = any.downcast_ref::<NSButton>().unwrap();
    assert_eq!(b.keyEquivalent().to_string(), "\r");
    el.set_key_equivalent("");
    assert_eq!(b.keyEquivalent().to_string(), "");
}

fn text_field_bordered_and_bezeled() {
    let _mtm = common::test_mtm();
    let el = Element::create("text_field");
    el.set_text_field_bordered(false);
    el.set_text_field_bezeled(false);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert!(!f.isBordered());
    assert!(!f.isBezeled());

    // NSTextField treats bordered + bezeled as mutually
    // exclusive: setting one switches off the other. Verify
    // each takes effect when set independently.
    el.set_text_field_bordered(true);
    assert!(f.isBordered());

    el.set_text_field_bezeled(true);
    assert!(f.isBezeled());
}

fn label_selectable() {
    let _mtm = common::test_mtm();
    let el = Element::create("label");
    el.set_selectable(true);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let f = any.downcast_ref::<NSTextField>().unwrap();
    assert!(f.isSelectable());
    el.set_selectable(false);
    assert!(!f.isSelectable());
}

fn slider_vertical_orientation() {
    let _mtm = common::test_mtm();
    let el = Element::create("slider");
    el.set_slider_vertical(true);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSSlider>().unwrap();
    assert!(s.isVertical());
}

fn slider_tick_marks_and_snaps() {
    let _mtm = common::test_mtm();
    let el = Element::create("slider");
    el.set_slider_tick_marks(5);
    el.set_slider_snaps_to_ticks(true);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSSlider>().unwrap();
    assert_eq!(s.numberOfTickMarks(), 5);
    assert!(s.allowsTickMarkValuesOnly());
}

fn pop_up_pulls_down_round_trip() {
    let _mtm = common::test_mtm();
    let el = Element::create("pop_up_button");
    el.set_pulls_down(true);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let p = any.downcast_ref::<NSPopUpButton>().unwrap();
    assert!(p.pullsDown());
    el.set_pulls_down(false);
    assert!(!p.pullsDown());
}

fn segment_style_round_trip() {
    use objc2_app_kit::{NSSegmentStyle, NSSegmentedControl};
    let _mtm = common::test_mtm();
    let el = Element::create("segmented_control");
    el.set_segment_style(NSSegmentStyle::Capsule);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let sc = any.downcast_ref::<NSSegmentedControl>().unwrap();
    assert_eq!(sc.segmentStyle(), NSSegmentStyle::Capsule);
}

fn date_picker_style_round_trip() {
    use objc2_app_kit::{NSDatePicker, NSDatePickerStyle};
    let _mtm = common::test_mtm();
    let el = Element::create("date_picker");
    el.set_date_picker_style(NSDatePickerStyle::ClockAndCalendar);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let dp = any.downcast_ref::<NSDatePicker>().unwrap();
    assert_eq!(
        dp.datePickerStyle(),
        NSDatePickerStyle::ClockAndCalendar
    );
}

fn date_picker_min_max_round_trip() {
    use objc2_app_kit::NSDatePicker;
    let _mtm = common::test_mtm();
    let el = Element::create("date_picker");
    let min = cocoa_dom::Date::from_unix_secs(1_000_000.0);
    let max = cocoa_dom::Date::from_unix_secs(2_000_000.0);
    el.set_date_picker_min(Some(min));
    el.set_date_picker_max(Some(max));

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let dp = any.downcast_ref::<NSDatePicker>().unwrap();
    let got_min = dp.minDate().map(|d| {
        cocoa_dom::Date::from_nsdate(&d).seconds_since_epoch
    });
    let got_max = dp.maxDate().map(|d| {
        cocoa_dom::Date::from_nsdate(&d).seconds_since_epoch
    });
    assert_eq!(got_min, Some(1_000_000.0));
    assert_eq!(got_max, Some(2_000_000.0));

    // Clear them.
    el.set_date_picker_min(None);
    el.set_date_picker_max(None);
    assert!(dp.minDate().is_none());
    assert!(dp.maxDate().is_none());
}

fn scroll_view_scroller_toggles() {
    use objc2_app_kit::NSScrollView;
    let _mtm = common::test_mtm();
    let el = Element::create("scroll_view");
    el.set_autohides_scrollers(true);
    el.set_has_horizontal_scroller(true);
    el.set_has_vertical_scroller(false);

    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let s = any.downcast_ref::<NSScrollView>().unwrap();
    assert!(s.autohidesScrollers());
    assert!(s.hasHorizontalScroller());
    assert!(!s.hasVerticalScroller());
}

fn progress_displayed_when_stopped_round_trip() {
    use objc2_app_kit::NSProgressIndicator;
    let _mtm = common::test_mtm();
    let el = Element::create("progress_indicator");
    el.set_progress_displayed_when_stopped(true);
    let v = el.ns_view();
    let any: &AnyObject = v.as_ref();
    let p = any.downcast_ref::<NSProgressIndicator>().unwrap();
    assert!(p.isDisplayedWhenStopped());
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
            "text_view_is_scroll_view_with_textview_inside",
            text_view_is_scroll_view_with_textview_inside,
        ),
        ("text_view_value_round_trips", text_view_value_round_trips),
        (
            "text_view_set_editable_round_trips",
            text_view_set_editable_round_trips,
        ),
        (
            "scroll_view_is_nsscrollview_with_doc_view",
            scroll_view_is_nsscrollview_with_doc_view,
        ),
        (
            "scroll_view_subview_parent_routes_to_doc_view",
            scroll_view_subview_parent_routes_to_doc_view,
        ),
        (
            "scroll_view_insert_routes_child_to_doc_view",
            scroll_view_insert_routes_child_to_doc_view,
        ),
        ("date_picker_is_nsdatepicker", date_picker_is_nsdatepicker),
        ("date_picker_value_round_trips", date_picker_value_round_trips),
        ("stepper_is_nsstepper", stepper_is_nsstepper),
        ("stepper_configure_and_round_trip", stepper_configure_and_round_trip),
        (
            "progress_indicator_is_nsprogressindicator",
            progress_indicator_is_nsprogressindicator,
        ),
        (
            "progress_indicator_value_and_max",
            progress_indicator_value_and_max,
        ),
        (
            "progress_indicator_indeterminate_toggles",
            progress_indicator_indeterminate_toggles,
        ),
        ("color_well_is_nscolorwell", color_well_is_nscolorwell),
        ("color_well_value_round_trips", color_well_value_round_trips),
        (
            "segmented_control_is_nssegmentedcontrol",
            segmented_control_is_nssegmentedcontrol,
        ),
        ("segmented_items_round_trip", segmented_items_round_trip),
        (
            "segmented_selection_round_trips",
            segmented_selection_round_trips,
        ),
        ("image_view_is_nsimageview", image_view_is_nsimageview),
        (
            "image_view_set_path_with_missing_file_clears_image",
            image_view_set_path_with_missing_file_clears_image,
        ),
        (
            "image_view_empty_path_clears_image",
            image_view_empty_path_clears_image,
        ),
        (
            "pop_up_button_is_nspopupbutton_pull_up",
            pop_up_button_is_nspopupbutton_pull_up,
        ),
        ("unknown_tag_falls_back_to_view", unknown_tag_falls_back_to_view),
        ("kind_is_always_element", kind_is_always_element),
        // Universal attrs
        ("alpha_round_trips_and_clamps", alpha_round_trips_and_clamps),
        ("tool_tip_set_and_clear", tool_tip_set_and_clear),
        // Text styling
        ("text_color_on_text_field", text_color_on_text_field),
        ("text_color_on_label", text_color_on_label),
        ("alignment_on_text_field", alignment_on_text_field),
        ("font_size_on_label", font_size_on_label),
        ("font_size_on_text_view", font_size_on_text_view),
        // Per-control
        ("button_bordered_round_trip", button_bordered_round_trip),
        (
            "button_key_equivalent_round_trip",
            button_key_equivalent_round_trip,
        ),
        ("text_field_bordered_and_bezeled", text_field_bordered_and_bezeled),
        ("label_selectable", label_selectable),
        ("slider_vertical_orientation", slider_vertical_orientation),
        ("slider_tick_marks_and_snaps", slider_tick_marks_and_snaps),
        ("pop_up_pulls_down_round_trip", pop_up_pulls_down_round_trip),
        ("segment_style_round_trip", segment_style_round_trip),
        ("date_picker_style_round_trip", date_picker_style_round_trip),
        ("date_picker_min_max_round_trip", date_picker_min_max_round_trip),
        ("scroll_view_scroller_toggles", scroll_view_scroller_toggles),
        (
            "progress_displayed_when_stopped_round_trip",
            progress_displayed_when_stopped_round_trip,
        ),
    ]);
}
