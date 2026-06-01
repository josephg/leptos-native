//! `Node` — a `Copy` handle (just a `NodeId`) into the ambient
//! thread-local node store.
//!
//! There is no `Rc`, no `SendWrapper`, no refcount: a `Node` is a bare
//! generational `NodeId` (`Copy + Send`). The backing `NSView`, Taffy
//! style, per-node meta and ObjC handler retains all live in the
//! per-thread `LayoutState<CocoaBackend>` (see [`crate::layout`] and
//! `renderer::layout`). Accessors fetch through the store by id; a
//! stale id (after the node was freed) resolves to `None`/no-op via the
//! generational key — weak-reference behavior for free.
//!
//! Lifecycle is explicit: [`CocoaElem::teardown`] removes the node and its
//! whole structural subtree from the store. There is no drop-driven
//! removal (a `Node` is `Copy`, so it has no `Drop`).

use super::layout::CocoaMeta;
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSButton, NSControl, NSTextField, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use leptos_native::renderer::{LayoutBackend, Node};
use send_wrapper::SendWrapper;
use taffy::Style;
use crate::dom::{event, layout, Color, Date, DatePickerStyle, KeyEvent, LineBreak, SegmentStyle, TextAlignment};
use super::layout::CocoaBackend;

/// The macOS port's node handle: a [`Node`] tagged with [`CocoaBackend`].
///
/// Structurally just a generational [`NodeId`] (`Copy + Send`). The
/// generic accessor surface (`id`, `view`, `with_style`, `with_style_mut`,
/// `with_tag`, `ptr_eq`, `from_id`) is inherent on [`Node`] and needs no
/// import. The **AppKit-specific** widget operations live on the
/// [`CocoaNodeExt`] extension trait below — inherent `impl` on the foreign
/// `Node<CocoaBackend>` alias isn't possible from this crate, so a local
/// trait carries them (orphan-safe: the trait is local).
///
/// All per-node state (the `NSView`, Taffy style, [`CocoaMeta`], ObjC
/// handler retains) lives in `LayoutState<CocoaBackend>`. Accessors read
/// through the store keyed by `id`.
pub type CocoaElem = Node<CocoaBackend>;

/// AppKit-specific operations on a [`CocoaElem`]. Bring this trait into
/// scope to call `.set_title()`, `.on_click()`, `CocoaElem::create_text()`,
/// etc. (The generic `.id()` / `.view()` / `.with_style()` surface is
/// inherent on [`Node`] and needs no import.)
///
/// The few `try_downcast` / `apply_font` / `read_font_*` entries are
/// internal helpers shared across the bodies; they ride along on the trait
/// because, post-newtype-elimination, there's no private inherent block to
/// hold them.
pub trait CocoaNodeExt: Copy {
    fn from_view<V>(
        view: Retained<V>,
        default_style: Style,
        default_meta: CocoaMeta,
    ) -> Self
    where
        V: AsRef<NSView> + Message;
    fn ns_view(self) -> Retained<NSView>;
    fn try_ns_view(self) -> Option<Retained<NSView>>;
    fn try_downcast<T>(self) -> Option<Retained<T>>
    where
        T: DowncastTarget;
    fn ns_view_retained(self) -> Retained<NSView>;
    fn teardown(self);
    fn with_meta<R>(self, f: impl FnOnce(&CocoaMeta) -> R) -> R;
    fn with_meta_mut<R>(self, f: impl FnOnce(&mut CocoaMeta) -> R) -> R;
    fn with_handlers_mut<R>(
        self,
        f: impl FnOnce(&mut event::NodeHandlers) -> R,
    ) -> R;
    fn create_container() -> Self;
    fn create_container_with(mtm: MainThreadMarker) -> Self;
    fn subview_parent(self) -> Retained<NSView>;
    fn insert_node(self, child: CocoaElem, marker: Option<CocoaElem>);
    fn remove_child(self, child: CocoaElem) -> Option<CocoaElem>;
    fn clear_children(self);
    fn set_title(self, value: &str);
    fn set_value(self, value: &str);
    fn set_placeholder(self, value: &str);
    fn set_enabled(self, value: bool);
    fn set_checked(self, value: bool);
    fn on_click(self, cb: impl FnMut() + 'static);
    fn on_action(self, cb: impl FnMut() + 'static);
    fn on_value_change(self, cb: impl FnMut() + Send + 'static);
    fn on_text_change(self, cb: impl FnMut(String) + 'static);
    fn on_hover(self, cb: impl FnMut(bool) + 'static);
    fn checked(self) -> bool;
    fn double_value(self) -> f64;
    fn set_double_value(self, v: f64);
    fn set_slider_min(self, v: f64);
    fn set_slider_max(self, v: f64);
    fn set_popup_items(self, items: &[String]);
    fn popup_selection(self) -> isize;
    fn set_popup_selection(self, idx: isize);
    fn set_segmented_items(self, items: &[String]);
    fn segmented_selection(self) -> isize;
    fn set_segmented_selection(self, idx: isize);
    fn set_alpha(self, alpha: f64);
    fn set_tool_tip(self, tip: &str);
    fn set_text_color(self, color: Color);
    fn set_text_alignment(self, alignment: TextAlignment);
    fn set_font_size(self, points: f64);
    fn set_bold(self, bold: bool);
    fn set_italic(self, italic: bool);
    fn apply_font(
        self,
        points: f64,
        traits: objc2_app_kit::NSFontDescriptorSymbolicTraits,
    );
    fn read_font_point_size(self) -> Option<f64>;
    fn read_font_traits(self) -> objc2_app_kit::NSFontDescriptorSymbolicTraits;
    fn set_button_bordered(self, bordered: bool);
    fn set_key_equivalent(self, key: &str);
    fn set_button_title_color(self, color: Color);
    fn set_button_sf_symbol(self, name: &str);
    fn set_intrinsic_width_from_content(self, on: bool);
    fn set_text_field_bordered(self, bordered: bool);
    fn set_text_field_bezeled(self, bezeled: bool);
    fn set_selectable(self, selectable: bool);
    fn set_line_break(self, mode: LineBreak);
    fn set_multiline(self, multi: bool);
    fn set_slider_vertical(self, vertical: bool);
    fn set_slider_tick_marks(self, count: usize);
    fn set_slider_snaps_to_ticks(self, snaps: bool);
    fn set_pulls_down(self, pulls_down: bool);
    fn set_segment_style(self, style: SegmentStyle);
    fn set_date_picker_style(self, style: DatePickerStyle);
    fn set_date_picker_min(self, d: Option<Date>);
    fn set_date_picker_max(self, d: Option<Date>);
    fn set_autohides_scrollers(self, autohides: bool);
    fn set_has_horizontal_scroller(self, has: bool);
    fn set_has_vertical_scroller(self, has: bool);
    fn set_scroll_axis(self, axis: layout::ScrollAxis);
    fn set_progress_displayed_when_stopped(self, shown: bool);
    fn date_picker_value(self) -> Date;
    fn set_date_picker_value(self, d: Date);
    fn stepper_value(self) -> f64;
    fn set_stepper_value(self, v: f64);
    fn configure_stepper(self, min: f64, max: f64, increment: f64);
    fn set_stepper_min(self, v: f64);
    fn set_stepper_max(self, v: f64);
    fn set_stepper_increment(self, v: f64);
    fn set_progress_value(self, v: f64);
    fn set_progress_indeterminate(self, indeterminate: bool);
    fn set_progress_max(self, max: f64);
    fn color_well_value(self) -> Color;
    fn set_color_well_value(self, color: Color);
    fn on_text_end_editing(self, cb: impl FnMut(String) + 'static);
    fn on_text_focus(self, cb: impl FnMut() + 'static);
    fn on_text_blur(self, cb: impl FnMut() + 'static);
    fn on_text_keydown(self, cb: impl FnMut(KeyEvent) + 'static);
    fn on_text_keyup(self, cb: impl FnMut(KeyEvent) + 'static);
    fn set_image_view_path(self, path: &str);
    fn set_image_view_bytes(self, bytes: Option<&[u8]>);
    fn set_image_view_sf_symbol(self, name: &str);
    fn set_image_view_tint(self, color: Color);
    fn on_text_view_change(self, cb: impl FnMut(String) + 'static);
    fn set_text_view_editable(self, editable: bool);
    fn text_view_value(self) -> Option<String>;
    fn focus(self) -> bool;
    fn blur(self) -> bool;
    fn create_text(content: &str) -> Self;
    fn create_text_with(content: &str, mtm: MainThreadMarker) -> Self;
    fn set_text(self, content: &str);
    fn create_placeholder() -> Self;
    fn create_placeholder_with(mtm: MainThreadMarker) -> Self;
}

impl CocoaNodeExt for CocoaElem {
    /// Allocate a fresh entry in the store and return a `Node` for it.
    /// The typed registration primitive: hand in a concrete NSView
    /// subclass, get back a `Node`.
    fn from_view<V>(
        view: Retained<V>,
        default_style: Style,
        default_meta: CocoaMeta,
    ) -> Self
    where
        V: AsRef<NSView> + Message,
    {
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(view) };
        let id = CocoaBackend::new_leaf(
            default_style,
            SendWrapper::new(view.clone()),
            default_meta,
            event::NodeHandlers::default(),
        );
        // Wire the handlers' view back-ref so teardown can nil
        // setTarget/setDelegate while the view is still alive.
        CocoaBackend::with_handlers_mut(id, |h| h.attach_view(view));
        Node::from_id(id)
    }

    /// Borrow the underlying NSView (owned clone). Main-thread only.
    /// Panics if the node is no longer in the store.
    fn ns_view(self) -> Retained<NSView> {
        CocoaBackend::view(self.id)
            .map(|sw| sw.take())
            .expect("Node id must exist in the store")
    }

    /// `Some(view)` if the node is still in the store, else `None`.
    fn try_ns_view(self) -> Option<Retained<NSView>> {
        CocoaBackend::view(self.id).map(|sw| sw.take())
    }

    /// Downcast the live NSView to `T`. `None` if the node is gone
    /// from the store OR the view isn't a `T`.
    ///
    /// Setters and readers go through this (not the panicking
    /// `ns_view()`) so a reactive effect that fires *after* the node
    /// was torn down is a graceful no-op rather than a panic. Under
    /// the `Copy`-`NodeId` model a `RenderEffect` closure captures
    /// only the id (it pins nothing), so an async-scheduled effect
    /// re-run can outlive its node.
    ///
    /// This is **defense-in-depth, not the primary fix**. The real fix
    /// is that `ElementState::unmount` drops `_effects` before tearing
    /// the node down (see `leptos_cocoa::cocoa::element`), which ends
    /// the effects' driver futures so they can't re-run on a freed
    /// node — fuzzing showed that alone drives late-fires to zero. We
    /// keep this guard anyway because a stray late-fire here would
    /// panic inside an async effect poll, which the runtime escalates
    /// to a process *abort* (not a catchable unwind) — far worse in
    /// production than a no-op. It also matches the web backend, where
    /// setting an attribute on a detached-but-alive node is harmless.
    /// Trade-off: a future regression of the unmount cleanup is
    /// swallowed silently here rather than failing loudly in the
    /// fuzzer.
    fn try_downcast<T>(self) -> Option<Retained<T>>
    where
        T: DowncastTarget,
    {
        self.try_ns_view().and_then(|v| downcast::<T>(&v))
    }

    /// Get a `Retained<NSView>` without panicking-vs-cloning concerns —
    /// kept for call-site parity with the old API.
    fn ns_view_retained(self) -> Retained<NSView> {
        self.ns_view()
    }

    /// Remove this node and its whole structural subtree from the
    /// store, and detach its NSView from its superview.
    fn teardown(self) {
        if let Some(view) = self.try_ns_view() {
            view.removeFromSuperview();
        }
        CocoaBackend::remove(self.id);
    }

    // ---- Accessor surface ------------------------------------------

    /// Borrow the node's [`CocoaMeta`] for read.
    fn with_meta<R>(self, f: impl FnOnce(&CocoaMeta) -> R) -> R {
        let meta = CocoaBackend::meta(self.id).unwrap_or_default();
        f(&meta)
    }

    /// Mutate the node's [`CocoaMeta`].
    fn with_meta_mut<R>(self, f: impl FnOnce(&mut CocoaMeta) -> R) -> R {
        let mut meta = CocoaBackend::meta(self.id).unwrap_or_default();
        let r = f(&mut meta);
        CocoaBackend::set_meta(self.id, meta);
        r
    }

    /// Mutate this node's per-node handler set in the store. Panics
    /// if the node isn't present (callers install handlers on live,
    /// mounted nodes).
    fn with_handlers_mut<R>(
        self,
        f: impl FnOnce(&mut event::NodeHandlers) -> R,
    ) -> R {
        CocoaBackend::with_handlers_mut(self.id, f)
            .expect("Node id must exist in the store")
    }

    /// Generic flipped container (FlippedView, default Taffy style).
    fn create_container() -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_container_with(mtm)
    }

    fn create_container_with(mtm: MainThreadMarker) -> Self {
        use super::{flipped_view::FlippedView, layout::Style};
        let view: Retained<NSView> = unsafe {
            Retained::cast_unchecked(FlippedView::new(mtm))
        };
        CocoaElem::from_view(view, Style::default(), CocoaMeta::default())
    }

    /// The NSView that *actually* parents this node's children. For
    /// `<scroll_view>` it's the NSScrollView's documentView.
    fn subview_parent(self) -> Retained<NSView> {
        let direct = self.ns_view();
        let routes_to_doc = self.with_meta(|m| m.is_scroll_view);
        if routes_to_doc {
            if let Some(scroll) =
                downcast::<objc2_app_kit::NSScrollView>(&direct)
            {
                if let Some(doc) = scroll.documentView() {
                    return doc;
                }
            }
        }
        direct
    }

    /// Insert `child` before `marker` in this element's child list.
    /// If `marker` is `None`, append.
    fn insert_node(self, child: CocoaElem, marker: Option<CocoaElem>) {
        let parent_retained = self.subview_parent();
        let parent: &NSView = &parent_retained;
        let child_view = child.ns_view();

        match marker {
            None => {
                parent.addSubview(&child_view);
                layout::attach_child(self, child);
            }
            Some(marker) => {
                let marker_view = marker.ns_view();
                splice_subview_before(parent, &child_view, &marker_view);
                let subviews = parent.subviews();
                let child_ptr: *const NSView = &*child_view;
                let mut child_index = 0_usize;
                for sv in subviews.iter() {
                    let sv_ptr: *const NSView = &*sv;
                    if sv_ptr == child_ptr {
                        break;
                    }
                    #[cfg(feature = "debug-overlay")]
                    {
                        if sv.tag() == super::debug_overlay::OVERLAY_TAG {
                            continue;
                        }
                    }
                    child_index += 1;
                }
                layout::insert_child_at(self, child, child_index);
            }
        }
    }

    /// Remove `child` from this element. Returns the node back if it was
    /// actually our child, otherwise `None`.
    fn remove_child(self, child: CocoaElem) -> Option<CocoaElem> {
        let parent_retained = self.subview_parent();
        let parent_ptr: *const NSView = &*parent_retained;
        let child_view = child.ns_view();
        let child_super = unsafe { child_view.superview() };
        let same_parent = match child_super {
            Some(sv) => {
                let sv_ptr: *const NSView = &*sv;
                sv_ptr == parent_ptr
            }
            None => false,
        };
        if !same_parent {
            return None;
        }
        child_view.removeFromSuperview();
        layout::detach_child(self, child);
        Some(child)
    }

    /// Remove every child (NSView level only).
    fn clear_children(self) {
        let parent_retained = self.subview_parent();
        let parent: &NSView = &parent_retained;
        let subs = parent.subviews();
        for sv in subs.iter() {
            sv.removeFromSuperview();
        }
    }

    /// Set the title on an NSButton. No-op on other view classes.
    fn set_title(self, value: &str) {
        if let Some(button) = self.try_downcast::<NSButton>() {
            let current = button.title().to_string();
            if current != value {
                button.setTitle(&NSString::from_str(value));
                layout::schedule_relayout(self);
            }
        }
    }

    /// Set the string value on an NSControl, or route to the
    /// `<text_view>`'s documentView. No-op on other view classes.
    fn set_value(self, value: &str) {
        let Some(view) = self.try_ns_view() else { return; };
        if let Some(control) = downcast::<NSControl>(&view) {
            let current = control.stringValue().to_string();
            if current != value {
                control.setStringValue(&NSString::from_str(value));
                layout::schedule_relayout(self);
            }
        } else if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(&view)
        {
            if let Some(doc) = scroll.documentView() {
                let any_doc: &AnyObject = &doc;
                if let Some(tv) =
                    any_doc.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    let current = tv.string().to_string();
                    if current != value {
                        tv.setString(&NSString::from_str(value));
                        layout::schedule_relayout(self);
                    }
                }
            }
        }
    }

    /// Set the placeholder string on an NSTextField. No-op otherwise.
    fn set_placeholder(self, value: &str) {
        if let Some(field) = self.try_downcast::<NSTextField>() {
            let current: String = field
                .placeholderString()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current != value {
                field.setPlaceholderString(Some(&NSString::from_str(value)));
                layout::schedule_relayout(self);
            }
        }
    }

    /// Toggle the enabled state on an NSControl. Diff-guarded.
    fn set_enabled(self, value: bool) {
        if let Some(control) = self.try_downcast::<NSControl>() {
            if control.isEnabled() != value {
                control.setEnabled(value);
            }
        }
    }

    /// Set the on/off state on an NSButton. No-op otherwise.
    fn set_checked(self, value: bool) {
        if let Some(button) = self.try_downcast::<NSButton>() {
            use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn};
            let want = if value {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            };
            if button.state() != want {
                button.setState(want);
            }
        }
    }

    /// Wire a click handler. No-op if not an NSControl.
    fn on_click(self, cb: impl FnMut() + 'static) {
        event::on_control_action(self, cb);
    }

    /// Wire a value-change (target/action) handler. No-op if not an
    /// NSControl.
    fn on_action(self, cb: impl FnMut() + 'static) {
        event::on_control_action(self, cb);
    }

    /// Unit-payload value-change callback (delegate fan-out for text
    /// fields, target/action otherwise).
    fn on_value_change(self, mut cb: impl FnMut() + Send + 'static) {
        if self.try_downcast::<NSTextField>().is_some() {
            event::on_text_field_change(self, move |_| cb());
            return;
        }
        event::on_control_action(self, cb);
    }

    /// Wire a per-keystroke text-change callback. No-op if not an
    /// NSTextField. Multiple handlers supported.
    fn on_text_change(self, cb: impl FnMut(String) + 'static) {
        event::on_text_field_change(self, cb);
    }

    /// Install hover tracking.
    fn on_hover(self, cb: impl FnMut(bool) + 'static) {
        event::on_hover(self, cb);
    }

    /// Read the on/off state of an NSButton. `false` for non-buttons.
    fn checked(self) -> bool {
        if let Some(button) = self.try_downcast::<NSButton>() {
            use objc2_app_kit::NSControlStateValueOn;
            return button.state() == NSControlStateValueOn;
        }
        false
    }

    /// Read the current `doubleValue` of an NSControl. 0.0 otherwise.
    fn double_value(self) -> f64 {
        if let Some(c) = self.try_downcast::<NSControl>() {
            return c.doubleValue();
        }
        0.0
    }

    /// Set the `doubleValue` on an NSControl. Diff-guarded.
    fn set_double_value(self, v: f64) {
        if let Some(c) = self.try_downcast::<NSControl>() {
            if (c.doubleValue() - v).abs() > f64::EPSILON {
                c.setDoubleValue(v);
            }
        }
    }

    /// Slider min.
    fn set_slider_min(self, v: f64) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = self.try_downcast::<NSSlider>() {
            s.setMinValue(v);
        }
    }

    /// Slider max.
    fn set_slider_max(self, v: f64) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = self.try_downcast::<NSSlider>() {
            s.setMaxValue(v);
        }
    }

    /// Replace the items list on an NSPopUpButton. No-op otherwise.
    fn set_popup_items(self, items: &[String]) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = self.try_downcast::<NSPopUpButton>() {
            p.removeAllItems();
            for it in items {
                p.addItemWithTitle(&NSString::from_str(it));
            }
        }
    }

    /// Currently-selected index on an NSPopUpButton (-1 otherwise).
    fn popup_selection(self) -> isize {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = self.try_downcast::<NSPopUpButton>() {
            return p.indexOfSelectedItem();
        }
        -1
    }

    /// Programmatically pick an item by index. Diff-guarded.
    fn set_popup_selection(self, idx: isize) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = self.try_downcast::<NSPopUpButton>() {
            if p.indexOfSelectedItem() != idx {
                p.selectItemAtIndex(idx);
            }
        }
    }

    /// Replace the labels on an NSSegmentedControl. No-op otherwise.
    fn set_segmented_items(self, items: &[String]) {
        use objc2_app_kit::NSSegmentedControl;
        let Some(view) = self.try_ns_view() else { return; };
        let Some(sc) = downcast::<NSSegmentedControl>(&view) else {
            return;
        };
        sc.setSegmentCount(items.len() as isize);
        for (i, label) in items.iter().enumerate() {
            sc.setLabel_forSegment(&NSString::from_str(label), i as isize);
        }
    }

    /// Currently-selected segment (-1 otherwise).
    fn segmented_selection(self) -> isize {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) = self.try_downcast::<NSSegmentedControl>() {
            return sc.selectedSegment();
        }
        -1
    }

    /// Programmatically pick a segment by index. Diff-guarded.
    fn set_segmented_selection(self, idx: isize) {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) = self.try_downcast::<NSSegmentedControl>() {
            if sc.selectedSegment() != idx {
                sc.setSelectedSegment(idx);
            }
        }
    }

    // -----------------------------------------------------------------
    // Universal NSView attributes
    // -----------------------------------------------------------------

    /// Set this view's opacity (0.0..=1.0). Diff-guarded.
    fn set_alpha(self, alpha: f64) {
        let Some(v) = self.try_ns_view() else { return; };
        let clamped = alpha.clamp(0.0, 1.0);
        let old = v.alphaValue();
        if (old - clamped).abs() <= f64::EPSILON {
            return;
        }
        #[cfg(feature = "animation")]
        let visible_opacity = {
            v.setWantsLayer(true);
            v.layer().map(|layer| {
                super::animation::presentation_or_model(
                    &layer, |l| l.opacity() as f64,
                )
            })
        };
        v.setAlphaValue(clamped);
        #[cfg(feature = "animation")]
        if let (Some(visible), Some(layer)) = (visible_opacity, v.layer()) {
            super::animation::animate_float(
                &layer, "opacity", visible, clamped,
            );
        }
    }

    /// Set this view's tool tip. Empty string removes it.
    fn set_tool_tip(self, tip: &str) {
        let Some(v) = self.try_ns_view() else { return; };
        if tip.is_empty() {
            v.setToolTip(None);
        } else {
            let s = NSString::from_str(tip);
            v.setToolTip(Some(&s));
        }
    }

    // -----------------------------------------------------------------
    // Text styling
    // -----------------------------------------------------------------

    /// Set the text color on a text-bearing view. No-op otherwise.
    fn set_text_color(self, color: Color) {
        let Some(view) = self.try_ns_view() else { return; };
        let nscolor = color.to_nscolor();

        if let Some(field) = downcast::<NSTextField>(&view) {
            field.setTextColor(Some(&nscolor));
            return;
        }
        if let Some(scroll) = downcast::<objc2_app_kit::NSScrollView>(&view) {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    tv.setTextColor(Some(&nscolor));
                }
            }
        }
    }

    /// Set text alignment on a text-bearing view. No-op otherwise.
    fn set_text_alignment(self, alignment: TextAlignment) {
        let Some(view) = self.try_ns_view() else { return; };

        if let Some(field) = downcast::<NSTextField>(&view) {
            field.setAlignment(alignment.0);
            return;
        }
        if let Some(scroll) = downcast::<objc2_app_kit::NSScrollView>(&view) {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    tv.setAlignment(alignment.0);
                }
            }
        }
    }

    /// Set the font size (in points); preserves symbolic traits.
    fn set_font_size(self, points: f64) {
        let traits = self.read_font_traits();
        self.apply_font(points, traits);
    }

    /// Toggle bold weight; preserves size + other traits.
    fn set_bold(self, bold: bool) {
        use objc2_app_kit::NSFontDescriptorSymbolicTraits;
        let pts = self
            .read_font_point_size()
            .unwrap_or_else(|| objc2_app_kit::NSFont::systemFontSize());
        let mut traits = self.read_font_traits();
        if bold {
            traits |= NSFontDescriptorSymbolicTraits::TraitBold;
        } else {
            traits &= !NSFontDescriptorSymbolicTraits::TraitBold;
        }
        self.apply_font(pts, traits);
    }

    /// Toggle italic; preserves size + other traits.
    fn set_italic(self, italic: bool) {
        use objc2_app_kit::NSFontDescriptorSymbolicTraits;
        let pts = self
            .read_font_point_size()
            .unwrap_or_else(|| objc2_app_kit::NSFont::systemFontSize());
        let mut traits = self.read_font_traits();
        if italic {
            traits |= NSFontDescriptorSymbolicTraits::TraitItalic;
        } else {
            traits &= !NSFontDescriptorSymbolicTraits::TraitItalic;
        }
        self.apply_font(pts, traits);
    }

    fn apply_font(
        self,
        points: f64,
        traits: objc2_app_kit::NSFontDescriptorSymbolicTraits,
    ) {
        use objc2_app_kit::NSFont;
        let plain = NSFont::systemFontOfSize(points);
        let font = if traits.is_empty() {
            plain
        } else {
            let base = plain.fontDescriptor();
            let with_traits = base.fontDescriptorWithSymbolicTraits(traits);
            NSFont::fontWithDescriptor_size(&with_traits, points)
                .unwrap_or(plain)
        };

        let Some(view) = self.try_ns_view() else { return; };
        if let Some(field) = downcast::<NSTextField>(&view) {
            field.setFont(Some(&font));
        } else if let Some(button) = downcast::<NSButton>(&view) {
            button.setFont(Some(&font));
        } else if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(&view)
        {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    tv.setFont(Some(&font));
                }
            }
        }
        layout::schedule_relayout(self);
    }

    fn read_font_point_size(self) -> Option<f64> {
        let view = self.try_ns_view()?;
        if let Some(field) = downcast::<NSTextField>(&view) {
            return field.font().map(|f| f.pointSize());
        }
        if let Some(button) = downcast::<NSButton>(&view) {
            return button.font().map(|f| f.pointSize());
        }
        None
    }

    fn read_font_traits(self) -> objc2_app_kit::NSFontDescriptorSymbolicTraits {
        use objc2_app_kit::NSFontDescriptorSymbolicTraits;
        let Some(view) = self.try_ns_view() else {
            return NSFontDescriptorSymbolicTraits::empty();
        };
        let font = if let Some(field) = downcast::<NSTextField>(&view) {
            field.font()
        } else if let Some(button) = downcast::<NSButton>(&view) {
            button.font()
        } else {
            None
        };
        match font {
            Some(f) => f.fontDescriptor().symbolicTraits(),
            None => NSFontDescriptorSymbolicTraits::empty(),
        }
    }

    // -----------------------------------------------------------------
    // Control-specific statics
    // -----------------------------------------------------------------

    /// Toggle whether an NSButton draws its bezel. No-op otherwise.
    fn set_button_bordered(self, bordered: bool) {
        if let Some(b) = self.try_downcast::<NSButton>() {
            b.setBordered(bordered);
        }
    }

    /// Set the keyboard shortcut for an NSButton. No-op otherwise.
    fn set_key_equivalent(self, key: &str) {
        if let Some(b) = self.try_downcast::<NSButton>() {
            b.setKeyEquivalent(&NSString::from_str(key));
        }
    }

    /// Apply a custom title color to an NSButton (`contentTintColor`).
    fn set_button_title_color(self, color: Color) {
        let Some(view) = self.try_ns_view() else { return; };
        let Some(button) = downcast::<NSButton>(&view) else {
            return;
        };
        let ns_color = color.to_nscolor();
        button.setContentTintColor(Some(&ns_color));
    }

    /// Render an SF Symbol as the button's image. Empty name clears it.
    fn set_button_sf_symbol(self, name: &str) {
        use objc2_app_kit::NSCellImagePosition;
        let Some(view) = self.try_ns_view() else { return; };
        let Some(button) = downcast::<NSButton>(&view) else {
            return;
        };
        if name.is_empty() {
            button.setImage(None);
            button.setImagePosition(NSCellImagePosition::NoImage);
            return;
        }
        let image = sf_symbol_image(name);
        let has_title = button.title().length() > 0;
        button.setImage(image.as_deref());
        button.setImagePosition(if has_title {
            NSCellImagePosition::ImageLeading
        } else {
            NSCellImagePosition::ImageOnly
        });
        layout::schedule_relayout(self);
    }

    /// Toggle the `intrinsic_width = FromContent` opt-in. No-op
    /// on non-NSTextField.
    fn set_intrinsic_width_from_content(self, on: bool) {
        if self.try_downcast::<NSTextField>().is_some() {
            layout::mark_intrinsic_width_from_content(self, on);
            layout::schedule_relayout(self);
        }
    }

    /// Toggle whether an NSTextField draws a border. No-op otherwise.
    fn set_text_field_bordered(self, bordered: bool) {
        if let Some(f) = self.try_downcast::<NSTextField>() {
            f.setBordered(bordered);
        }
    }

    /// Toggle whether an NSTextField draws its bezel. No-op otherwise.
    fn set_text_field_bezeled(self, bezeled: bool) {
        if let Some(f) = self.try_downcast::<NSTextField>() {
            f.setBezeled(bezeled);
        }
    }

    /// Toggle whether a label can be selected. No-op otherwise.
    fn set_selectable(self, selectable: bool) {
        if let Some(f) = self.try_downcast::<NSTextField>() {
            f.setSelectable(selectable);
        }
    }

    /// Set the line-break behaviour on a text view. No-op otherwise.
    fn set_line_break(self, mode: LineBreak) {
        use objc2_app_kit::NSLineBreakMode;
        let wraps = matches!(
            mode.0,
            NSLineBreakMode::ByWordWrapping | NSLineBreakMode::ByCharWrapping
        );
        let Some(view) = self.try_ns_view() else { return; };
        if let Some(f) = downcast::<NSTextField>(&view) {
            f.setUsesSingleLineMode(!wraps);
            f.cell()
                .expect("NSTextField always has a cell")
                .setLineBreakMode(mode.0);
            f.setMaximumNumberOfLines(0);
            layout::schedule_relayout(self);
            return;
        }
        if let Some(scroll) = downcast::<objc2_app_kit::NSScrollView>(&view) {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    if let Some(container) = unsafe { tv.textContainer() } {
                        container.setLineBreakMode(mode.0);
                    }
                    layout::schedule_relayout(self);
                }
            }
        }
    }

    /// Shorthand for word-wrap / truncate-tail.
    fn set_multiline(self, multi: bool) {
        self.set_line_break(if multi {
            LineBreak::WORD_WRAP
        } else {
            LineBreak::TRUNCATE_TAIL
        });
    }

    /// Switch an NSSlider orientation. No-op otherwise.
    fn set_slider_vertical(self, vertical: bool) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = self.try_downcast::<NSSlider>() {
            s.setVertical(vertical);
        }
    }

    /// Set tick-mark count on an NSSlider. No-op otherwise.
    fn set_slider_tick_marks(self, count: usize) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = self.try_downcast::<NSSlider>() {
            s.setNumberOfTickMarks(count as isize);
        }
    }

    /// Toggle snap-to-tick on an NSSlider. No-op otherwise.
    fn set_slider_snaps_to_ticks(self, snaps: bool) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = self.try_downcast::<NSSlider>() {
            s.setAllowsTickMarkValuesOnly(snaps);
        }
    }

    /// Switch an NSPopUpButton between popup / pull-down. No-op otherwise.
    fn set_pulls_down(self, pulls_down: bool) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = self.try_downcast::<NSPopUpButton>() {
            p.setPullsDown(pulls_down);
        }
    }

    /// Set an NSSegmentedControl's visual style. No-op otherwise.
    fn set_segment_style(self, style: SegmentStyle) {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) = self.try_downcast::<NSSegmentedControl>() {
            sc.setSegmentStyle(style.0);
        }
    }

    /// Set NSDatePicker's visual style. No-op otherwise.
    fn set_date_picker_style(self, style: DatePickerStyle) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = self.try_downcast::<NSDatePicker>() {
            dp.setDatePickerStyle(style.0);
        }
    }

    /// Constrain an NSDatePicker's min date.
    fn set_date_picker_min(self, d: Option<Date>) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = self.try_downcast::<NSDatePicker>() {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMinDate(nd.as_deref());
        }
    }

    fn set_date_picker_max(self, d: Option<Date>) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = self.try_downcast::<NSDatePicker>() {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMaxDate(nd.as_deref());
        }
    }

    /// Toggle auto-hiding of an NSScrollView's scrollers.
    fn set_autohides_scrollers(self, autohides: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = self.try_downcast::<NSScrollView>() {
            s.setAutohidesScrollers(autohides);
        }
    }

    /// Show/hide an NSScrollView's horizontal scroller.
    fn set_has_horizontal_scroller(self, has: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = self.try_downcast::<NSScrollView>() {
            s.setHasHorizontalScroller(has);
        }
    }

    /// Show/hide an NSScrollView's vertical scroller.
    fn set_has_vertical_scroller(self, has: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = self.try_downcast::<NSScrollView>() {
            s.setHasVerticalScroller(has);
        }
    }

    /// Configure an `<scroll_view>`'s scroll axis. No-op otherwise.
    fn set_scroll_axis(self, axis: layout::ScrollAxis) {
        use layout::ScrollAxis;
        use taffy::FlexDirection;
        if !self.with_meta(|m| m.is_scroll_view) {
            return;
        }
        self.with_meta_mut(|m| m.scroll_axis = axis);

        self.with_style_mut(|s| {
            s.flex_direction = match axis {
                ScrollAxis::Vertical | ScrollAxis::Both => FlexDirection::Column,
                ScrollAxis::Horizontal => FlexDirection::Row,
            };
        });

        let wrapper = self.with_meta(|m| m.child_taffy_parent);
        if let Some(wid) = wrapper {
            CocoaBackend::set_style(
                wid,
                layout::build_scroll_wrapper_style(axis),
            );
        }

        use objc2_app_kit::NSScrollView;
        if let Some(s) = self.try_downcast::<NSScrollView>() {
            match axis {
                ScrollAxis::Vertical => {
                    s.setHasVerticalScroller(true);
                    s.setHasHorizontalScroller(false);
                }
                ScrollAxis::Horizontal => {
                    s.setHasVerticalScroller(false);
                    s.setHasHorizontalScroller(true);
                }
                ScrollAxis::Both => {
                    s.setHasVerticalScroller(true);
                    s.setHasHorizontalScroller(true);
                }
            }
        }
    }

    /// Toggle whether an NSProgressIndicator stays visible when stopped.
    fn set_progress_displayed_when_stopped(self, shown: bool) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) = self.try_downcast::<NSProgressIndicator>() {
            p.setDisplayedWhenStopped(shown);
        }
    }

    /// Read the current value of a `<date_picker>`.
    fn date_picker_value(self) -> Date {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = self.try_downcast::<NSDatePicker>() {
            let d = dp.dateValue();
            return Date::from_nsdate(&d);
        }
        Date::now()
    }

    /// Set the date shown in a `<date_picker>`. Diff-guarded.
    fn set_date_picker_value(self, d: Date) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = self.try_downcast::<NSDatePicker>() {
            let current = dp.dateValue();
            let current_secs = current.timeIntervalSince1970();
            if (current_secs - d.seconds_since_epoch).abs() > f64::EPSILON {
                dp.setDateValue(&d.to_nsdate());
            }
        }
    }

    /// Read the value of a `<stepper>`. 0.0 otherwise.
    fn stepper_value(self) -> f64 {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            return s.doubleValue();
        }
        0.0
    }

    /// Set the value of a `<stepper>`. Diff-guarded.
    fn set_stepper_value(self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            if (s.doubleValue() - v).abs() > f64::EPSILON {
                s.setDoubleValue(v);
            }
        }
    }

    /// Configure a `<stepper>`'s min/max/increment in one call.
    fn configure_stepper(self, min: f64, max: f64, increment: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            s.setMinValue(min);
            s.setMaxValue(max);
            s.setIncrement(increment);
        }
    }

    fn set_stepper_min(self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            s.setMinValue(v);
        }
    }

    fn set_stepper_max(self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            s.setMaxValue(v);
        }
    }

    fn set_stepper_increment(self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = self.try_downcast::<NSStepper>() {
            s.setIncrement(v);
        }
    }

    /// Set the `value` of a `<progress_indicator>`.
    fn set_progress_value(self, v: f64) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) = self.try_downcast::<NSProgressIndicator>() {
            p.setDoubleValue(v);
        }
    }

    /// Switch a `<progress_indicator>` between determinate / spinner.
    fn set_progress_indeterminate(self, indeterminate: bool) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) = self.try_downcast::<NSProgressIndicator>() {
            p.setIndeterminate(indeterminate);
            unsafe {
                if indeterminate {
                    p.startAnimation(None);
                } else {
                    p.stopAnimation(None);
                }
            }
        }
    }

    /// Set the progress max value. Default 1.0.
    fn set_progress_max(self, max: f64) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) = self.try_downcast::<NSProgressIndicator>() {
            p.setMaxValue(max);
        }
    }

    /// Read the current color from an `<color_well>`.
    fn color_well_value(self) -> Color {
        use objc2_app_kit::NSColorWell;
        if let Some(cw) = self.try_downcast::<NSColorWell>() {
            let c = cw.color();
            return Color::from_nscolor(&c)
                .unwrap_or(Color::BLACK);
        }
        Color::BLACK
    }

    /// Set the color shown in an `<color_well>`. No-op otherwise.
    fn set_color_well_value(self, color: Color) {
        use objc2_app_kit::NSColorWell;
        if let Some(cw) = self.try_downcast::<NSColorWell>() {
            cw.setColor(&color.to_nscolor());
        }
    }

    /// Wire a commit-edit callback. No-op on non-NSTextField.
    fn on_text_end_editing(self, cb: impl FnMut(String) + 'static) {
        event::on_text_field_end_editing(self, cb);
    }

    /// Wire a focus-gained callback. No-op on non-NSTextField.
    fn on_text_focus(self, cb: impl FnMut() + 'static) {
        event::on_text_field_focus(self, cb);
    }

    /// Wire a focus-lost callback. No-op on non-NSTextField.
    fn on_text_blur(self, cb: impl FnMut() + 'static) {
        event::on_text_field_blur(self, cb);
    }

    /// Wire a keydown observer on a text field. No-op otherwise.
    fn on_text_keydown(self, cb: impl FnMut(KeyEvent) + 'static) {
        event::on_text_field_keydown(self, cb);
    }

    /// Wire a keyup observer on a text field. No-op otherwise.
    fn on_text_keyup(self, cb: impl FnMut(KeyEvent) + 'static) {
        event::on_text_field_keyup(self, cb);
    }

    /// Load an image into an `<image_view>` from a file path.
    fn set_image_view_path(self, path: &str) {
        use objc2_app_kit::{NSImage, NSImageView};
        let Some(view) = self.try_ns_view() else { return; };
        let Some(iv) = downcast::<NSImageView>(&view) else {
            return;
        };
        if path.is_empty() {
            iv.setImage(None);
            return;
        }
        use objc2::AllocAnyThread;
        let path_str = NSString::from_str(path);
        let image =
            NSImage::initWithContentsOfFile(NSImage::alloc(), &path_str);
        iv.setImage(image.as_deref());
        layout::schedule_relayout(self);
    }

    /// Load an image into an `<image_view>` from in-memory bytes.
    fn set_image_view_bytes(self, bytes: Option<&[u8]>) {
        use objc2_app_kit::{NSImage, NSImageView};
        use objc2_foundation::NSData;
        let Some(view) = self.try_ns_view() else { return; };
        let Some(iv) = downcast::<NSImageView>(&view) else {
            return;
        };
        let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
            iv.setImage(None);
            layout::schedule_relayout(self);
            return;
        };
        use objc2::AllocAnyThread;
        let data = NSData::with_bytes(bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data);
        iv.setImage(image.as_deref());
        layout::schedule_relayout(self);
    }

    /// Set an `<image_view>` to render an SF Symbol by name.
    fn set_image_view_sf_symbol(self, name: &str) {
        use objc2_app_kit::NSImageView;
        let Some(view) = self.try_ns_view() else { return; };
        let Some(iv) = downcast::<NSImageView>(&view) else {
            return;
        };
        let image = sf_symbol_image(name);
        iv.setImage(image.as_deref());
        layout::schedule_relayout(self);
    }

    /// Set an image view's tint color.
    fn set_image_view_tint(self, color: Color) {
        use objc2_app_kit::NSImageView;
        let Some(view) = self.try_ns_view() else { return; };
        let Some(iv) = downcast::<NSImageView>(&view) else {
            return;
        };
        iv.setContentTintColor(Some(&color.to_nscolor()));
    }

    /// Wire a change observer on the NSTextView inside a `<text_view>`.
    fn on_text_view_change(self, cb: impl FnMut(String) + 'static) {
        event::on_text_view_change(self, cb);
    }

    /// Set the editability of the NSTextView inside a `<text_view>`.
    fn set_text_view_editable(self, editable: bool) {
        let Some(view) = self.try_ns_view() else { return; };
        let Some(scroll) = downcast::<objc2_app_kit::NSScrollView>(&view)
        else {
            return;
        };
        let Some(doc) = scroll.documentView() else { return };
        let any_doc: &AnyObject = &doc;
        if let Some(tv) =
            any_doc.downcast_ref::<objc2_app_kit::NSTextView>()
        {
            if tv.isEditable() != editable {
                tv.setEditable(editable);
            }
        }
    }

    /// Read the value of a `<text_view>`. `None` for non-text_view.
    fn text_view_value(self) -> Option<String> {
        let view = self.try_ns_view()?;
        let scroll = downcast::<objc2_app_kit::NSScrollView>(&view)?;
        let doc = scroll.documentView()?;
        let any_doc: &AnyObject = &doc;
        let tv = any_doc.downcast_ref::<objc2_app_kit::NSTextView>()?;
        Some(tv.string().to_string())
    }

    /// Make this element the window's first responder (focus).
    fn focus(self) -> bool {
        let Some(view) = self.try_ns_view() else { return false; };
        let Some(window) = view.window() else { return false };
        let responder: &objc2_app_kit::NSResponder = &view;
        window.makeFirstResponder(Some(responder))
    }

    /// Resign first-responder status (clears focus window-wide).
    fn blur(self) -> bool {
        let Some(view) = self.try_ns_view() else { return false; };
        let Some(window) = view.window() else { return false };
        window.makeFirstResponder(None)
    }

    // ---------------------------------------------------------------------
    // text-label & placeholder constructors
    // ---------------------------------------------------------------------

    /// Build a text-label Node — a non-editable, non-bordered
    /// NSTextField (AppKit's "label" configuration).
    fn create_text(content: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_text_with(content, mtm)
    }

    fn create_text_with(content: &str, mtm: MainThreadMarker) -> Self {
        let label = NSTextField::labelWithString(
            &NSString::from_str(content),
            mtm,
        );
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(label) };

        let mut style = layout::Style::default();
        style.flex_shrink = 0.0;

        CocoaElem::from_view(view, style, CocoaMeta::default())
    }

    /// Update the displayed string on a text-label Node.
    fn set_text(self, content: &str) {
        if let Some(field) = self.try_downcast::<NSTextField>() {
            field.setStringValue(&NSString::from_str(content));
        }
        layout::schedule_relayout(self);
    }

    /// Build a placeholder Node — a hidden, zero-sized, absolutely-
    /// positioned NSView used as a stable mount anchor.
    fn create_placeholder() -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_placeholder_with(mtm)
    }

    fn create_placeholder_with(mtm: MainThreadMarker) -> Self {
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0)),
        );
        view.setHidden(true);

        let mut style = layout::Style::default();
        style.position = layout::Position::Absolute;
        style.size.width = layout::Dimension::length(0.0);
        style.size.height = layout::Dimension::length(0.0);

        CocoaElem::from_view(view, style, CocoaMeta::default())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Best-effort downcast of an `&NSView` to a more specific subclass.
pub(crate) fn downcast<T>(view: &NSView) -> Option<Retained<T>>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>().map(|r| r.retain())
}

/// Load an SF Symbol by name with a default size/weight configuration.
pub(crate) fn sf_symbol_image(
    name: &str,
) -> Option<Retained<objc2_app_kit::NSImage>> {
    use objc2_app_kit::{
        NSFontWeightRegular, NSImage, NSImageSymbolConfiguration,
    };
    if name.is_empty() {
        return None;
    }
    let name_ns = NSString::from_str(name);
    let raw = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &name_ns, None,
    )?;
    let cfg = unsafe {
        NSImageSymbolConfiguration::configurationWithPointSize_weight(
            16.0,
            NSFontWeightRegular,
        )
    };
    raw.imageWithSymbolConfiguration(&cfg).or(Some(raw))
}

/// Insert `child` immediately before `marker` in `parent`'s subview array.
fn splice_subview_before(parent: &NSView, child: &NSView, marker: &NSView) {
    parent.addSubview_positioned_relativeTo(
        child,
        NSWindowOrderingMode::Below,
        Some(marker),
    );
}
