//! Typed per-control [`CocoaElem`] constructors.
//!
//! Each function here allocates a concrete AppKit view subclass
//! (NSButton / NSTextField / NSScrollView / ...), builds its
//! default Taffy [`Style`], and registers it in `tree` via
//! [`CocoaElem::from_view`]. Every typed builder in `leptos_cocoa`
//! calls exactly one of these from its `Render::build`.
//!
//! This replaces the old tag-string dispatch
//! (`CocoaNode::create(tree, "button")` → giant `match tag` in
//! `node.rs`). Wins:
//!
//! 1. Static-typed end to end — the builder knows it wants an
//!    NSButton, calls [`CocoaElem::create_button`], gets back both an
//!    `CocoaNode` and `Retained<NSButton>`. No string round-trip.
//! 2. Dead-code elimination works — a binary that doesn't use
//!    `<stepper>` doesn't carry NSStepper's construction code.
//! 3. Adding a new control is one function here + one builder, not
//!    "one function here + one builder + a new arm in an 18-way
//!    match in node.rs".
//!
//! Lives in `cocoa_dom` (rather than alongside the builders in
//! `leptos_cocoa`) so the per-port test suite under
//! `cocoa/dom/tests/` can drive these directly without depending
//! on the renderer crate.

use crate::dom::{event::NodeHandlers, flipped_view::FlippedView, layout, layout::{
    build_scroll_wrapper_style, scroll_view_document, CocoaBackend,
    CocoaMeta, Dimension, FlexDirection, ScrollAxis,
    Style
}, node::CocoaElem};
use objc2::{rc::Retained, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBorderType, NSButton, NSColorWell, NSDatePicker, NSImageScaling,
    NSImageView, NSPopUpButton, NSProgressIndicator, NSScrollView,
    NSSecureTextField, NSSegmentedControl, NSSlider, NSStepper,
    NSTextField, NSTextView, NSView,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use renderer::LayoutBackend;
use send_wrapper::SendWrapper;

fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("cocoa_dom must run on the main thread")
}

/// Sentinel frame — Taffy overwrites at layout time.
fn zero_frame() -> NSRect {
    NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0))
}

/// Default style for leaf controls (text fields, sliders, pickers,
/// etc.): never flex-shrink. Clipping a control's intrinsic content
/// looks broken — truncated button titles, half-rendered checkboxes,
/// labels overflowing into siblings.
fn leaf_style() -> Style {
    let mut s = Style::default();
    s.flex_shrink = 0.0;
    s
}

impl CocoaElem {
    /// Push button. Use `buttonWithTitle:target:action:` rather than
    /// `initWithFrame:` — the former produces a properly-styled push
    /// button (rounded bezel, ~32px tall, sensible intrinsic size).
    /// Direct `initWithFrame` gave us a button with a default bezel
    /// whose intrinsic was a too-small 20px tall × text-only wide, so
    /// titles like "Reset" rendered as "Rese".
    ///
    /// Title and target/action are set later via attribute setters /
    /// `on_click(...)`.
    pub fn create_button() -> (CocoaElem, Retained<NSButton>) {
        let mtm = mtm();
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(""),
                None,
                None,
                mtm,
            )
        };
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(button.clone()) };
        let el = CocoaElem::from_view(view,
                                      Style::default(),
                                      CocoaMeta::default(),
        );
        (el, button)
    }

    /// Checkbox — NSButton in checkbox style.
    /// `checkboxWithTitle:target:action:` pre-configures the cell
    /// (checkbox bezel, sensible intrinsic). Title and target/action
    /// set later.
    ///
    /// `flex_shrink=0`: clipping a checkbox label looks bad.
    pub fn create_checkbox() -> (CocoaElem, Retained<NSButton>) {
        let mtm = mtm();
        let button = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str(""),
                None,
                None,
                mtm,
            )
        };
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(button.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, button)
    }

    /// Wrapping multi-line label. `wrappingLabelWithString:` is
    /// AppKit's dedicated multiline-label initializer (10.12+). It
    /// pre-configures the cell for word-wrapping; combined with
    /// `preferredMaxLayoutWidth` set during measure (see
    /// [`crate::layout::measure_leaf`]) the field reports a wrapped
    /// intrinsic content size for whatever width the parent gives it.
    ///
    /// No hardcoded size — measured via NSTextField's intrinsic.
    /// Never shrink: NSTextField doesn't clip its text content, so a
    /// frame shorter than the text height results in text overflowing
    /// into siblings' space.
    pub fn create_label() -> (CocoaElem, Retained<NSTextField>) {
        let mtm = mtm();
        let label =
            NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(label.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, label)
    }

    /// Editable single-line text field. Measured via NSTextField
    /// intrinsic; never shrinks (editable content shouldn't get
    /// clipped by sibling overlap).
    pub fn create_text_field() -> (CocoaElem, Retained<NSTextField>) {
        let mtm = mtm();
        let tf =
            NSTextField::initWithFrame(NSTextField::alloc(mtm), zero_frame());
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(tf.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, tf)
    }

    /// Password-style text field. NSSecureTextField IS-A NSTextField,
    /// so downstream code that downcasts to NSTextField still works.
    pub fn create_secure_text_field() -> (CocoaElem, Retained<NSSecureTextField>) {
        let mtm = mtm();
        let tf = NSSecureTextField::initWithFrame(
            NSSecureTextField::alloc(mtm),
            zero_frame(),
        );
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(tf.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, tf)
    }

    /// Continuous-update slider. `setContinuous(true)` makes the
    /// slider fire target/action on every drag-update, not just on
    /// mouse-up — matches the web `<input type=range>` event firing
    /// while sliding.
    ///
    /// Sliders have a defined intrinsic height; the parent decides
    /// width via the cross-axis stretch.
    pub fn create_slider() -> (CocoaElem, Retained<NSSlider>) {
        let mtm = mtm();
        let slider =
            NSSlider::initWithFrame(NSSlider::alloc(mtm), zero_frame());
        slider.setContinuous(true);
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(slider.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, slider)
    }

    /// Pop-up button. `pullsDown=false` → menu-style (current
    /// selection shown in the bezel). The builder may flip to
    /// pull-down later via `setPullsDown`.
    pub fn create_pop_up_button() -> (CocoaElem, Retained<NSPopUpButton>) {
        let mtm = mtm();
        let p = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            zero_frame(),
            false,
        );
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(p.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, p)
    }

    pub fn create_date_picker() -> (CocoaElem, Retained<NSDatePicker>) {
        let mtm = mtm();
        let dp = NSDatePicker::initWithFrame(
            NSDatePicker::alloc(mtm),
            zero_frame(),
        );
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(dp.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, dp)
    }

    /// Numeric +/- stepper.
    ///
    /// `setValueWraps(false)`: clamp at min/max (web-shaped); user
    /// can flip via setValueWraps if they want wrap-around behavior.
    ///
    /// `setAutorepeat(true)`: fire on every drag tick (continuous),
    /// matches slider's default — consistent expectation for live-
    /// update controls.
    pub fn create_stepper() -> (CocoaElem, Retained<NSStepper>) {
        let mtm = mtm();
        let st =
            NSStepper::initWithFrame(NSStepper::alloc(mtm), zero_frame());
        st.setValueWraps(false);
        st.setAutorepeat(true);
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(st.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, st)
    }

    /// Bar-style determinate progress indicator (0..1). User can flip
    /// to indeterminate (spinner) via `.indeterminate(true)` on the
    /// builder.
    pub fn create_progress_indicator() -> (CocoaElem, Retained<NSProgressIndicator>) {
        let mtm = mtm();
        let pi = NSProgressIndicator::initWithFrame(
            NSProgressIndicator::alloc(mtm),
            zero_frame(),
        );
        pi.setMinValue(0.0);
        pi.setMaxValue(1.0);
        pi.setIndeterminate(false);
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(pi.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, pi)
    }

    pub fn create_color_well() -> (CocoaElem, Retained<NSColorWell>) {
        let mtm = mtm();
        let cw = NSColorWell::initWithFrame(
            NSColorWell::alloc(mtm),
            zero_frame(),
        );
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(cw.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, cw)
    }

    pub fn create_segmented_control() -> (CocoaElem, Retained<NSSegmentedControl>) {
        let mtm = mtm();
        let sc = NSSegmentedControl::initWithFrame(
            NSSegmentedControl::alloc(mtm),
            zero_frame(),
        );
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(sc.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, sc)
    }

    /// Scaling-down-only image view. Images larger than the frame fit
    /// inside; smaller images render at native size.
    pub fn create_image_view() -> (CocoaElem, Retained<NSImageView>) {
        let mtm = mtm();
        let iv = NSImageView::initWithFrame(
            NSImageView::alloc(mtm),
            zero_frame(),
        );
        iv.setImageScaling(NSImageScaling::ScaleProportionallyDown);
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(iv.clone()) };
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, iv)
    }

    /// Multi-line text editing surface. NSTextView is multi-line,
    /// rich-text-capable text editing. Standard AppKit pattern is to
    /// embed it inside an NSScrollView so overflow scrolls (NSTextView
    /// itself doesn't scroll). We wrap the scroll view and treat the
    /// contained text view as an internal implementation detail;
    /// setters route through `documentView()` (see
    /// `set_string_attribute` for `StringAttr::Value`).
    ///
    /// Limitations of this v1:
    ///   * No event hooks (NSTextViewDelegate is a separate protocol
    ///     from NSControlTextEditingDelegate). Add when needed.
    ///   * Plain text only (`setRichText(false)`).
    ///
    /// Returns the *outer* NSScrollView; the inner NSTextView is
    /// reachable via `scroll.documentView()`.
    pub fn create_text_view() -> (CocoaElem, Retained<NSScrollView>) {
        let mtm = mtm();
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_frame());
        scroll.setHasVerticalScroller(true);
        scroll.setHasHorizontalScroller(false);
        scroll.setBorderType(NSBorderType::BezelBorder);

        let tv =
            NSTextView::initWithFrame(NSTextView::alloc(mtm), zero_frame());
        tv.setEditable(true);
        tv.setSelectable(true);
        tv.setRichText(false);
        tv.setImportsGraphics(false);
        scroll.setDocumentView(Some(&tv));

        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(scroll.clone()) };
        // Don't shrink past content; multi-line editing surfaces
        // shouldn't get squeezed by sibling overlap.
        let el =
            CocoaElem::from_view(view, leaf_style(), CocoaMeta::default());
        (el, scroll)
    }

    /// User-scrollable container. NSScrollView wrapping a FlippedView
    /// documentView. Children added via `insert_node` are routed to
    /// the documentView via [`CocoaElem::subview_parent`]. The scroll
    /// view's *outer* frame is whatever the parent gives it (the
    /// viewport); the documentView is sized separately in a second
    /// `compute_layout` pass with `MaxContent` height — see
    /// `compute_layout_scroll_views` in layout.rs.
    ///
    /// Allocates a second Taffy leaf (an internal-leaf, refcount=0)
    /// backed by the NSScrollView's documentView. Children added at
    /// the AppKit layer are routed to this wrapper at the Taffy layer
    /// (see `taffy_child_parent` in layout.rs), so the user's
    /// children are laid out inside it.
    ///
    /// Default style is the "shrinkable scroll container" pattern:
    /// `flex-basis: 0; min-height: 0; overflow: hidden`. Without
    /// flex_basis=0, the flex algorithm would start scroll_view at
    /// its content size (e.g. 538 tall for a list of 30 rows), and
    /// flex-grow can't bring it back down — outer ancestors would
    /// then grow past the window. With flex_basis=0 and min_size=0,
    /// scroll_view collapses to nothing by default and only the
    /// user's flex_grow / explicit height grows it back to the
    /// viewport.
    pub fn create_scroll_view() -> (CocoaElem, Retained<NSScrollView>) {
        let mtm = mtm();
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), zero_frame());
        scroll.setHasVerticalScroller(true);
        scroll.setHasHorizontalScroller(false);
        scroll.setBorderType(NSBorderType::NoBorder);

        let doc: Retained<NSView> =
            unsafe { Retained::cast_unchecked(FlippedView::new(mtm)) };
        scroll.setDocumentView(Some(&doc));

        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(scroll.clone()) };

        let mut style = Style::default();
        style.flex_direction = FlexDirection::Column;
        style.flex_basis = Dimension::length(0.0);
        style.min_size.height = Dimension::length(0.0);
        style.overflow = taffy::Point {
            x: taffy::Overflow::Hidden,
            y: taffy::Overflow::Hidden,
        };

        let mut meta = CocoaMeta::default();
        meta.is_scroll_view = true;

        let el = CocoaElem::from_view(view, style, meta);

        // Internal Taffy wrapper backed by the document view. No
        // `Node` owns it; it's a structural child of the scroll_view,
        // so the cascade in `renderer::remove` frees it automatically
        // when the scroll_view goes away.
        if let Some(doc_view) = scroll_view_document(&el.ns_view()) {
            let wrapper_style = build_scroll_wrapper_style(ScrollAxis::Vertical);
            let parent_id = el.id();
            let wrapper_id = CocoaBackend::new_leaf(
                wrapper_style,
                SendWrapper::new(doc_view),
                CocoaMeta::default(),
                NodeHandlers::default(),
            );
            CocoaBackend::add_child(parent_id, wrapper_id);
            el.with_meta_mut(|m| {
                m.child_taffy_parent = Some(wrapper_id);
            });
        }

        (el, scroll)
    }

    /// `<grid>` — 2-D grid container backed by Taffy's grid algorithm.
    /// Template tracks / gap / placement attrs are applied by the
    /// higher-level builder; this just establishes the container.
    pub fn create_grid() -> CocoaElem {
        let mtm = mtm();
        let view: Retained<NSView> =
            unsafe { Retained::cast_unchecked(FlippedView::new(mtm)) };
        let mut style = Style::default();
        style.display = layout::Display::Grid;
        CocoaElem::from_view(view, style, CocoaMeta::default())
    }
}
