//! Typed per-control [`UikitElem`] constructors.
//!
//! Each function allocates a concrete UIKit view subclass (UIButton,
//! UILabel, UIScrollView, ...), builds its default Taffy [`Style`],
//! and registers it in `tree` via [`UikitElem::from_view`]. Every typed
//! builder in `leptos_uikit` calls exactly one of these from its
//! `Render::build`.
//!
//! Same shape as cocoa's `make_view.rs` — replaces the old tag-string
//! match.

use crate::{
    event::IosNodeHandlers,
    layout::{Dimension, FlexDirection, IosMeta, Style},
    node::UikitElem,
};

#[allow(unused_imports)]
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_ui_kit::{
    UIButton, UIButtonType, UIColorWell, UIDatePicker, UIImageView,
    UILabel, UIProgressView, UIProgressViewStyle, UIScrollView,
    UISegmentedControl, UISlider, UIStepper, UISwitch, UITextBorderStyle,
    UITextField, UITextInputTraits, UITextView, UIView, UIViewContentMode,
};

fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("ios_dom must run on the main thread")
}

fn zero_frame() -> NSRect {
    NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0))
}

fn leaf_style() -> Style {
    let mut s = Style::default();
    s.flex_shrink = 0.0;
    s
}

impl UikitElem {
    /// System-style push button (UIButton with type System). Targets/
    /// titles wired later via attribute setters / `on_click`.
    pub fn create_button() -> (UikitElem, Retained<UIButton>) {
        let b = UIButton::buttonWithType(UIButtonType::System, mtm());
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(b.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, b)
    }

    /// UISwitch — boolean toggle.
    pub fn create_switch() -> (UikitElem, Retained<UISwitch>) {
        let sw = UISwitch::initWithFrame(UISwitch::alloc(mtm()), zero_frame());
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(sw.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, sw)
    }

    pub fn create_label() -> (UikitElem, Retained<UILabel>) {
        let l = UILabel::initWithFrame(UILabel::alloc(mtm()), zero_frame());
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(l.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, l)
    }

    /// Editable single-line text field with rounded-rect bezel.
    pub fn create_text_field() -> (UikitElem, Retained<UITextField>) {
        let tf = UITextField::initWithFrame(UITextField::alloc(mtm()), zero_frame());
        tf.setBorderStyle(UITextBorderStyle::RoundedRect);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(tf.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, tf)
    }

    pub fn create_secure_text_field() -> (UikitElem, Retained<UITextField>) {
        let tf = UITextField::initWithFrame(UITextField::alloc(mtm()), zero_frame());
        tf.setSecureTextEntry(true);
        tf.setBorderStyle(UITextBorderStyle::RoundedRect);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(tf.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, tf)
    }

    /// Continuous-update slider — `setContinuous(true)`.
    pub fn create_slider() -> (UikitElem, Retained<UISlider>) {
        let sl = UISlider::initWithFrame(UISlider::alloc(mtm()), zero_frame());
        sl.setContinuous(true);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(sl.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, sl)
    }

    pub fn create_date_picker() -> (UikitElem, Retained<UIDatePicker>) {
        let dp = UIDatePicker::initWithFrame(
            UIDatePicker::alloc(mtm()),
            zero_frame(),
        );
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(dp.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, dp)
    }

    /// +/- numeric stepper. `setAutorepeat(true)` + `setContinuous(true)`
    /// — fire on every drag tick.
    pub fn create_stepper() -> (UikitElem, Retained<UIStepper>) {
        let st = UIStepper::initWithFrame(UIStepper::alloc(mtm()), zero_frame());
        st.setAutorepeat(true);
        st.setContinuous(true);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(st.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, st)
    }

    /// `UIProgressView` (Default style). Named for cocoa parity
    /// (`<progress_indicator>`).
    pub fn create_progress_indicator() -> (UikitElem, Retained<UIProgressView>) {
        let pv = UIProgressView::initWithProgressViewStyle(
            UIProgressView::alloc(mtm()),
            UIProgressViewStyle::Default,
        );
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(pv.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, pv)
    }

    /// UIImageView with aspect-fit scaling.
    pub fn create_image_view() -> (UikitElem, Retained<UIImageView>) {
        let iv = UIImageView::initWithFrame(
            UIImageView::alloc(mtm()),
            zero_frame(),
        );
        iv.setContentMode(UIViewContentMode::ScaleAspectFit);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(iv.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, iv)
    }

    /// Menu-style button — UIButton with `setShowsMenuAsPrimaryAction`.
    pub fn create_pop_up_button() -> (UikitElem, Retained<UIButton>) {
        let b = UIButton::buttonWithType(UIButtonType::System, mtm());
        b.setShowsMenuAsPrimaryAction(true);
        b.setChangesSelectionAsPrimaryAction(true);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(b.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, b)
    }

    pub fn create_color_well() -> (UikitElem, Retained<UIColorWell>) {
        let cw = UIColorWell::initWithFrame(
            UIColorWell::alloc(mtm()),
            zero_frame(),
        );
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(cw.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, cw)
    }

    pub fn create_segmented_control() -> (UikitElem, Retained<UISegmentedControl>) {
        let sc = UISegmentedControl::initWithFrame(
            UISegmentedControl::alloc(mtm()),
            zero_frame(),
        );
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(sc.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, sc)
    }

    /// User-scrollable container — UIScrollView with a content UIView
    /// added as the first subview. Children added via `insert_node`
    /// route to the content view.
    pub fn create_scroll_view() -> (UikitElem, Retained<UIScrollView>) {
        let scroll = UIScrollView::initWithFrame(
            UIScrollView::alloc(mtm()),
            zero_frame(),
        );
        scroll.setShowsVerticalScrollIndicator(true);
        scroll.setShowsHorizontalScrollIndicator(false);

        let content = UIView::initWithFrame(UIView::alloc(mtm()), zero_frame());
        scroll.addSubview(&content);

        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(scroll.clone()) };
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Column;
        s.flex_basis = Dimension::length(0.0);
        s.min_size.height = Dimension::length(0.0);
        s.overflow = taffy::Point {
            x: taffy::Overflow::Hidden,
            y: taffy::Overflow::Hidden,
        };

        let mut meta = IosMeta::default();
        meta.is_scroll_view = true;

        let n = UikitElem::from_view(view, s, meta);
        (n, scroll)
    }

    /// Multi-line text editing surface — UITextView, editable + selectable.
    pub fn create_text_view() -> (UikitElem, Retained<UITextView>) {
        let tv = UITextView::initWithFrame(UITextView::alloc(mtm()), zero_frame());
        tv.setEditable(true);
        tv.setSelectable(true);
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(tv.clone()) };
        let n = UikitElem::from_view(view, leaf_style(), IosMeta::default());
        (n, tv)
    }

    /// `<stack_view>` / `<vstack>` / `<view>` — vertical UIView
    /// container.
    pub fn create_vstack() -> UikitElem {
        let view: Retained<UIView> =
            UIView::initWithFrame(UIView::alloc(mtm()), zero_frame());
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Column;
        UikitElem::from_view(view, s, IosMeta::default())
    }

    /// `<hstack>` — horizontal UIView container.
    pub fn create_hstack() -> UikitElem {
        let view: Retained<UIView> =
            UIView::initWithFrame(UIView::alloc(mtm()), zero_frame());
        let mut s = Style::default();
        s.flex_direction = FlexDirection::Row;
        UikitElem::from_view(view, s, IosMeta::default())
    }

    /// `<grid>` — UIView container with Taffy `Display::Grid`.
    pub fn create_grid() -> UikitElem {
        let view: Retained<UIView> =
            UIView::initWithFrame(UIView::alloc(mtm()), zero_frame());
        let mut s = Style::default();
        s.display = crate::layout::Display::Grid;
        UikitElem::from_view(view, s, IosMeta::default())
    }
}

// Keep IosNodeHandlers as a referenced import target for completeness
// (used by Node::from_view internally via the arena).
#[allow(dead_code)]
fn _keep(_: IosNodeHandlers) {}
