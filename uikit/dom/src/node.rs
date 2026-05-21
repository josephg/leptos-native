//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `Retained<UIView>`.
//!
//! Each `Node` is a single `Rc<NodeInner>` that carries:
//!   * the tree it lives in (`TreeRef`),
//!   * its arena `NodeId`,
//!   * a cached `Retained<UIView>` for cheap `&UIView` access,
//!   * an `is_borrowed` flag controlling whether `Drop` decrefs the
//!     arena entry.
//!
//! All style / meta / handler state lives in the arena's `NodeData`.
//! Accessors (`with_style`, `with_meta`, `with_handlers_mut`) route
//! straight to the arena. Allocation is eager: `Element::create_with`
//! takes a `tree: &TreeRef` and allocates an arena entry up front.
//! See `cocoa/dom/src/node.rs` for the longer rationale.
//!
//! See the crate-level docs for the threading contract.

use crate::layout::{IosMeta, NodeId, Style};
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_ui_kit::{UIButton, UIControl, UITextField, UIView};
use objc2_foundation::NSString;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, rc::Rc};
use crate::layout::IosBackend;
use renderer::LayoutBackend;

/// A handle into the ambient node store — structurally just a
/// generational [`NodeId`]. `Copy + Send`.
///
/// All per-node state (the `UIView`, Taffy style, [`IosMeta`], ObjC
/// handler retains) lives in `LayoutState<IosBackend>`; accessors read
/// through the store keyed by `id`. A `Node` owns nothing — capturing
/// one in a handler closure can't form a retain cycle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UikitElem {
    pub(crate) id: NodeId,
}

impl UikitElem {
    /// Typed registration primitive: hand in a concrete UIView
    /// subclass, get back a `Node`.
    pub fn from_view<V>(
        view: Retained<V>,
        default_style: Style,
        default_meta: IosMeta,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(view) };
        let id = IosBackend::new_leaf(
            default_style,
            SendWrapper::new(view.clone()),
            default_meta,
            crate::event::IosNodeHandlers::default(),
        );
        // Wire the handlers' view back-ref so teardown can nil
        // setDelegate / removeAllTargets while the view is still alive.
        IosBackend::with_handlers_mut(id, |h| h.attach_view(view));
        UikitElem { id }
    }

    /// Wrap an existing store id as a `Node`.
    pub fn from_id(id: NodeId) -> Self {
        UikitElem { id }
    }

    /// The node's `NodeId`.
    pub fn id(self) -> NodeId {
        self.id
    }

    /// The underlying UIView (owned clone). Main-thread only. Panics
    /// if the node is no longer in the store.
    pub fn ui_view(self) -> Retained<UIView> {
        IosBackend::view(self.id)
            .map(|sw| sw.take())
            .expect("Node id must exist in the store")
    }

    /// `Some(view)` if the node is still in the store.
    pub fn try_ui_view(self) -> Option<Retained<UIView>> {
        IosBackend::view(self.id).map(|sw| sw.take())
    }

    /// Downcast the live UIView to `T`. `None` if the node is gone
    /// from the store OR the view isn't a `T`.
    ///
    /// Setters and readers go through this (not the panicking
    /// `ui_view()`) so a reactive effect that fires *after* the node
    /// was torn down is a graceful no-op rather than a panic. Under
    /// the `Copy`-`NodeId` model a `RenderEffect` closure captures
    /// only the id (it pins nothing), so an async-scheduled effect
    /// re-run can outlive its node.
    ///
    /// This is **defense-in-depth, not the primary fix**. The real fix
    /// is that `ElementState::unmount` drops `_effects` before tearing
    /// the node down (see `leptos_uikit::ios::element`), which ends the
    /// effects' driver futures so they can't re-run on a freed node. We
    /// keep this guard anyway because a stray late-fire here would
    /// panic inside an async effect poll, which the runtime escalates
    /// to a process *abort* (not a catchable unwind) — far worse in
    /// production than a no-op. It also matches the web backend, where
    /// setting an attribute on a detached-but-alive node is harmless.
    /// Trade-off: a future regression of the unmount cleanup is
    /// swallowed silently here rather than failing loudly.
    fn try_downcast<T>(self) -> Option<Retained<T>>
    where
        T: DowncastTarget,
    {
        self.try_ui_view().and_then(|v| downcast::<T>(&v))
    }

    pub fn ui_view_retained(self) -> Retained<UIView> {
        self.ui_view()
    }

    pub fn ptr_eq(self, other: UikitElem) -> bool {
        self.id == other.id
    }

    /// Remove this node (and its structural subtree) from the store
    /// and detach its UIView.
    pub fn teardown(self) {
        if let Some(view) = self.try_ui_view() {
            view.removeFromSuperview();
        }
        IosBackend::remove(self.id);
    }

    // ---- Accessor surface ------------------------------------------

    pub fn with_style<R>(self, f: impl FnOnce(&Style) -> R) -> R {
        let style = IosBackend::style(self.id).unwrap_or_default();
        f(&style)
    }

    pub fn with_style_mut<R>(self, f: impl FnOnce(&mut Style) -> R) -> R {
        let mut style = IosBackend::style(self.id).unwrap_or_default();
        let r = f(&mut style);
        IosBackend::set_style(self.id, style);
        r
    }

    pub fn with_meta<R>(self, f: impl FnOnce(&IosMeta) -> R) -> R {
        let meta = IosBackend::meta(self.id).unwrap_or_default();
        f(&meta)
    }

    pub fn with_meta_mut<R>(self, f: impl FnOnce(&mut IosMeta) -> R) -> R {
        let mut meta = IosBackend::meta(self.id).unwrap_or_default();
        let r = f(&mut meta);
        IosBackend::set_meta(self.id, meta);
        r
    }

    /// Mutate this node's per-node handler set in the store. Panics if
    /// the node isn't present (handlers install on live nodes).
    pub fn with_handlers_mut<R>(
        self,
        f: impl FnOnce(&mut crate::event::IosNodeHandlers) -> R,
    ) -> R {
        IosBackend::with_handlers_mut(self.id, f)
            .expect("Node id must exist in the store")
    }
}

// ---------------------------------------------------------------------
// Node — typed-builder / renderer-protocol surface
// ---------------------------------------------------------------------

impl UikitElem {
    /// Generic UIView container (default style). Used by
    /// `<view>` / `<stack>` builders and by `RootViewController`
    /// for the content root.
    pub fn create_container() -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_container_with(mtm)
    }

    pub fn create_container_with(mtm: MainThreadMarker) -> Self {
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let view: Retained<UIView> = UIView::initWithFrame(UIView::alloc(mtm), frame);
        UikitElem::from_view(view, Style::default(), IosMeta::default())
    }


    /// The UIView that *actually* parents this element's children.
    pub fn subview_parent(self) -> Retained<UIView> {
        let direct = self.ui_view();
        let routes_to_doc = self.with_meta(|m| m.is_scroll_view);
        if routes_to_doc {
            if let Some(scroll) =
                downcast::<objc2_ui_kit::UIScrollView>(&direct)
            {
                let subs = scroll.subviews();
                if subs.count() > 0 {
                    return subs.objectAtIndex(0);
                }
            }
        }
        direct
    }

    pub fn insert_node(self, child: UikitElem, marker: Option<UikitElem>) {
        let parent_retained = self.subview_parent();
        let parent: &UIView = &parent_retained;
        let child_view = child.ui_view();

        match marker {
            None => {
                parent.addSubview(&child_view);
                crate::layout::attach_child(self, child);
            }
            Some(marker) => {
                let marker_view = marker.ui_view();
                parent.insertSubview_belowSubview(&child_view, &marker_view);
                let subviews = parent.subviews();
                let child_ptr: *const UIView = &*child_view;
                let mut child_index = subviews.len();
                for (i, sv) in subviews.iter().enumerate() {
                    let sv_ptr: *const UIView = &*sv;
                    if sv_ptr == child_ptr {
                        child_index = i;
                        break;
                    }
                }
                crate::layout::insert_child_at(
                    self,
                    child,
                    child_index,
                );
            }
        }
    }

    pub fn remove_child(self, child: UikitElem) -> Option<UikitElem> {
        let parent_retained = self.subview_parent();
        let parent_ptr: *const UIView = &*parent_retained;
        let child_view = child.ui_view();
        let child_super = child_view.superview();
        let same_parent = match child_super {
            Some(sv) => {
                let sv_ptr: *const UIView = &*sv;
                sv_ptr == parent_ptr
            }
            None => false,
        };
        if !same_parent {
            return None;
        }
        child_view.removeFromSuperview();
        crate::layout::detach_child(self, child);
        Some(child)
    }

    pub fn clear_children(self) {
        let parent_retained = self.subview_parent();
        let parent: &UIView = &parent_retained;
        let subs = parent.subviews();
        for sv in subs.iter() {
            sv.removeFromSuperview();
        }
    }

    /// Set the title on a UIButton (Normal state) or the text on a
    /// UILabel. No-op on other classes.
    pub fn set_title(self, value: &str) {
        let Some(view) = self.try_ui_view() else { return; };
        let mut changed = false;
        if let Some(button) = downcast::<UIButton>(&view) {
            let current = button
                .titleForState(objc2_ui_kit::UIControlState::Normal)
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current != value {
                button.setTitle_forState(
                    Some(&NSString::from_str(value)),
                    objc2_ui_kit::UIControlState::Normal,
                );
                changed = true;
            }
        }
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(&view) {
            let current = label.text().map(|s| s.to_string()).unwrap_or_default();
            if current != value {
                label.setText(Some(&NSString::from_str(value)));
                changed = true;
            }
        }
        if changed {
            crate::layout::schedule_relayout(self);
        }
    }

    /// Set the text/value on a UITextField or UITextView. No-op on
    /// other classes.
    pub fn set_value(self, value: &str) {
        let Some(view) = self.try_ui_view() else { return; };
        let mut changed = false;
        if let Some(field) = downcast::<UITextField>(&view) {
            let current = field.text().map(|s| s.to_string()).unwrap_or_default();
            if current != value {
                field.setText(Some(&NSString::from_str(value)));
                changed = true;
            }
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(&view) {
            let current = tv.text().to_string();
            if current != value {
                tv.setText(Some(&NSString::from_str(value)));
                changed = true;
            }
        }
        if changed {
            crate::layout::schedule_relayout(self);
        }
    }

    /// Set the placeholder on a UITextField. No-op on other classes.
    pub fn set_placeholder(self, value: &str) {
        let Some(view) = self.try_ui_view() else { return; };
        if let Some(field) = downcast::<UITextField>(&view) {
            let current: String = field
                .placeholder()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current != value {
                field.setPlaceholder(Some(&NSString::from_str(value)));
                crate::layout::schedule_relayout(self);
            }
        }
    }

    /// Toggle UIView visibility.
    pub fn set_hidden(self, value: bool) {
        let Some(view) = self.try_ui_view() else { return; };
        if view.isHidden() != value {
            view.setHidden(value);
        }
    }

    /// Toggle user-interaction / enabled state. Sets
    /// `isUserInteractionEnabled` on the UIView; for UIControl
    /// subclasses, also sets `isEnabled`.
    pub fn set_enabled(self, value: bool) {
        let Some(view) = self.try_ui_view() else { return; };
        if view.isUserInteractionEnabled() != value {
            view.setUserInteractionEnabled(value);
        }
        if let Some(control) = downcast::<UIControl>(&view) {
            if control.isEnabled() != value {
                control.setEnabled(value);
            }
        }
    }

    /// Set the on/off state on a UISwitch (animated). No-op on
    /// other classes.
    pub fn set_checked(self, value: bool) {
        let Some(view) = self.try_ui_view() else { return; };
        if let Some(sw) = downcast::<objc2_ui_kit::UISwitch>(&view) {
            if sw.isOn() != value {
                sw.setOn_animated(value, true);
            }
        }
    }

    pub fn on_click(self, cb: impl FnMut() + 'static) {
        let Some(view) = self.try_ui_view() else { return; };
        if downcast::<UIControl>(&view).is_some() {
            crate::event::on_control_action(self, cb);
        } else {
            crate::event::on_tap_gesture(self, cb);
        }
    }

    pub fn on_action(self, cb: impl FnMut() + 'static) {
        crate::event::on_control_action(self, cb);
    }

    pub fn on_value_change(self, mut cb: impl FnMut() + Send + 'static) {
        if self.try_downcast::<UITextField>().is_some() {
            crate::event::on_text_field_change(self, move |_| cb());
            return;
        }
        crate::event::on_control_action(self, cb);
    }

    pub fn on_text_change(self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_field_change(self, cb);
    }

    pub fn on_text_end_editing(self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_field_end_editing(self, cb);
    }

    pub fn on_text_focus(self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_focus(self, cb);
    }

    pub fn on_text_blur(self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_blur(self, cb);
    }

    pub fn on_text_keydown(
        self,
        _cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        // Deferred: UIKeyCommand + pressesBegan:
    }

    pub fn on_text_keyup(
        self,
        _cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
    }

    pub fn checked(self) -> bool {
        if let Some(sw) = self.try_downcast::<objc2_ui_kit::UISwitch>() {
            return sw.isOn();
        }
        false
    }

    pub fn double_value(self) -> f64 {
        if let Some(sl) = self.try_downcast::<objc2_ui_kit::UISlider>() {
            return sl.value() as f64;
        }
        0.0
    }

    pub fn set_double_value(self, v: f64) {
        if let Some(sl) = self.try_downcast::<objc2_ui_kit::UISlider>() {
            let current = sl.value() as f64;
            if (current - v).abs() > f64::EPSILON {
                sl.setValue(v as f32);
            }
        }
    }

    pub fn set_slider_min(self, v: f64) {
        if let Some(sl) = self.try_downcast::<objc2_ui_kit::UISlider>() {
            sl.setMinimumValue(v as f32);
        }
    }

    pub fn set_slider_max(self, v: f64) {
        if let Some(sl) = self.try_downcast::<objc2_ui_kit::UISlider>() {
            sl.setMaximumValue(v as f32);
        }
    }

    pub fn set_segmented_items(self, items: &[String]) {
        let Some(sc) =
            self.try_downcast::<objc2_ui_kit::UISegmentedControl>()
        else {
            return;
        };
        let current = sc.numberOfSegments();
        for _ in 0..current {
            sc.removeSegmentAtIndex_animated(0, false);
        }
        for (i, label) in items.iter().enumerate() {
            sc.insertSegmentWithTitle_atIndex_animated(
                Some(&NSString::from_str(label)),
                i,
                false,
            );
        }
    }

    pub fn segmented_selection(self) -> isize {
        if let Some(sc) =
            self.try_downcast::<objc2_ui_kit::UISegmentedControl>()
        {
            return sc.selectedSegmentIndex();
        }
        -1
    }

    pub fn set_segmented_selection(self, idx: isize) {
        if let Some(sc) =
            self.try_downcast::<objc2_ui_kit::UISegmentedControl>()
        {
            if sc.selectedSegmentIndex() != idx {
                sc.setSelectedSegmentIndex(idx);
            }
        }
    }

    pub fn set_popup_items(
        self,
        items: &[String],
        selected_idx: usize,
        on_select: impl FnMut(usize) + 'static,
    ) {
        use objc2_ui_kit::{UIAction, UIMenu, UIMenuElement, UIMenuElementState};
        let Some(button) = self.try_downcast::<UIButton>() else {
            return;
        };
        let mtm = MainThreadMarker::new()
            .expect("set_popup_items must run on the main thread");

        let shared = Rc::new(RefCell::new(on_select));

        let actions: Vec<Retained<UIMenuElement>> = items
            .iter()
            .enumerate()
            .map(|(i, title)| {
                let title_ns = NSString::from_str(title);
                let cb = shared.clone();
                let action_handler = block2::RcBlock::new(
                    move |_: std::ptr::NonNull<UIAction>| {
                        cb.borrow_mut()(i);
                    },
                );
                let handler_ptr: *mut block2::Block<dyn Fn(std::ptr::NonNull<UIAction>) + 'static> =
                    &*action_handler as *const _ as *mut _;
                let action = unsafe {
                    UIAction::actionWithTitle_image_identifier_handler(
                        &title_ns,
                        None,
                        None,
                        handler_ptr,
                        mtm,
                    )
                };
                if i == selected_idx {
                    action.setState(UIMenuElementState::On);
                }
                let element: Retained<UIMenuElement> =
                    unsafe { Retained::cast_unchecked(action) };
                element
            })
            .collect();

        let ns_array = objc2_foundation::NSArray::from_retained_slice(&actions);
        let menu = UIMenu::menuWithChildren(&ns_array, mtm);
        button.setMenu(Some(&menu));

        if let Some(t) = items.get(selected_idx) {
            let ns = NSString::from_str(t);
            button.setTitle_forState(
                Some(&ns),
                objc2_ui_kit::UIControlState::Normal,
            );
            crate::layout::schedule_relayout(self);
        }
    }

    pub fn set_popup_selection(self, items: &[String], idx: usize) {
        let Some(button) = self.try_downcast::<UIButton>() else {
            return;
        };
        if let Some(t) = items.get(idx) {
            let ns = NSString::from_str(t);
            let current = button
                .titleForState(objc2_ui_kit::UIControlState::Normal)
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current.as_str() != t {
                button.setTitle_forState(
                    Some(&ns),
                    objc2_ui_kit::UIControlState::Normal,
                );
                crate::layout::schedule_relayout(self);
            }
        }
    }

    pub fn set_color_well_value(self, color: crate::Color) {
        use objc2_ui_kit::UIColorWell;
        let Some(cw) = self.try_downcast::<UIColorWell>() else {
            return;
        };
        cw.setSelectedColor(Some(&color.to_uicolor()));
    }

    pub fn color_well_value(self) -> Option<crate::Color> {
        use objc2_ui_kit::UIColorWell;
        let cw = self.try_downcast::<UIColorWell>()?;
        let c = cw.selectedColor()?;
        crate::Color::from_uicolor(&c)
    }

    pub fn on_color_change(
        self,
        mut cb: impl FnMut(crate::Color) + 'static,
    ) {
        use objc2_ui_kit::UIColorWell;
        let Some(cw) = self.try_downcast::<UIColorWell>() else {
            return;
        };
        let cw_for_cb: Retained<UIColorWell> = cw.retain();
        crate::event::on_control_action(self, move || {
            if let Some(c) = cw_for_cb.selectedColor() {
                if let Some(color) = crate::Color::from_uicolor(&c) {
                    cb(color);
                }
            }
        });
    }

    pub fn set_alpha(self, alpha: f64) {
        let Some(v) = self.try_ui_view() else { return; };
        let clamped = alpha.clamp(0.0, 1.0);
        if (v.alpha() - clamped).abs() > f64::EPSILON {
            v.setAlpha(clamped);
        }
    }

    pub fn set_background_color(self, color: Option<crate::Color>) {
        let Some(v) = self.try_ui_view() else { return; };
        match color {
            Some(c) => v.setBackgroundColor(Some(&c.to_uicolor())),
            None => v.setBackgroundColor(None),
        }
    }

    pub fn set_corner_radius(self, radius: f64) {
        let Some(__v) = self.try_ui_view() else { return; };
        let layer = __v.layer();
        if (layer.cornerRadius() - radius).abs() > f64::EPSILON {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(radius > 0.0);
        }
    }

    pub fn set_border_width(self, width: f64) {
        let Some(__v) = self.try_ui_view() else { return; };
        let layer = __v.layer();
        if (layer.borderWidth() - width).abs() > f64::EPSILON {
            layer.setBorderWidth(width);
        }
    }

    pub fn set_border_color(self, color: Option<crate::Color>) {
        let Some(__v) = self.try_ui_view() else { return; };
        let layer = __v.layer();
        match color {
            Some(c) => {
                let cg = unsafe { c.to_uicolor().CGColor() };
                layer.setBorderColor(Some(&cg));
            }
            None => layer.setBorderColor(None),
        }
    }

    pub fn set_text_color(self, color: crate::Color) {
        let Some(view) = self.try_ui_view() else { return; };
        let uicolor = color.to_uicolor();

        if let Some(field) = downcast::<UITextField>(&view) {
            field.setTextColor(Some(&uicolor));
            return;
        }
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(&view) {
            unsafe { label.setTextColor(Some(&uicolor)) };
            return;
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(&view) {
            tv.setTextColor(Some(&uicolor));
        }
    }

    pub fn set_text_alignment(self, alignment: crate::TextAlignment) {
        let Some(view) = self.try_ui_view() else { return; };

        if let Some(field) = downcast::<UITextField>(&view) {
            field.setTextAlignment(alignment.0);
            return;
        }
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(&view) {
            label.setTextAlignment(alignment.0);
            return;
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(&view) {
            tv.setTextAlignment(alignment.0);
        }
    }

    pub fn set_font_size(self, points: f64) {
        use objc2_ui_kit::UIFont;
        let font = UIFont::systemFontOfSize(points);

        let Some(view) = self.try_ui_view() else { return; };
        let mut applied = false;
        if let Some(field) = downcast::<UITextField>(&view) {
            field.setFont(Some(&font));
            applied = true;
        } else if let Some(label) = downcast::<objc2_ui_kit::UILabel>(&view) {
            unsafe { label.setFont(Some(&font)) };
            applied = true;
        } else if let Some(button) = downcast::<UIButton>(&view) {
            if let Some(title_label) = button.titleLabel() {
                unsafe { title_label.setFont(Some(&font)) };
                applied = true;
            }
        } else if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(&view) {
            tv.setFont(Some(&font));
            applied = true;
        }
        if applied {
            crate::layout::schedule_relayout(self);
        }
    }

    pub fn set_text_field_bordered(self, bordered: bool) {
        if let Some(f) = self.try_downcast::<UITextField>() {
            use objc2_ui_kit::UITextBorderStyle;
            f.setBorderStyle(if bordered {
                UITextBorderStyle::RoundedRect
            } else {
                UITextBorderStyle::None
            });
        }
    }

    pub fn set_text_field_bezeled(self, bezeled: bool) {
        self.set_text_field_bordered(bezeled);
    }

    pub fn set_slider_vertical(self, _vertical: bool) {}
    pub fn set_slider_tick_marks(self, _count: usize) {}
    pub fn set_slider_snaps_to_ticks(self, _snaps: bool) {}

    pub fn set_date_picker_style(self, style: crate::DatePickerStyle) {
        if let Some(dp) =
            self.try_downcast::<objc2_ui_kit::UIDatePicker>()
        {
            dp.setPreferredDatePickerStyle(style.0);
        }
    }

    pub fn set_date_picker_min(self, d: Option<crate::Date>) {
        if let Some(dp) =
            self.try_downcast::<objc2_ui_kit::UIDatePicker>()
        {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMinimumDate(nd.as_deref());
        }
    }

    pub fn set_date_picker_max(self, d: Option<crate::Date>) {
        if let Some(dp) =
            self.try_downcast::<objc2_ui_kit::UIDatePicker>()
        {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMaximumDate(nd.as_deref());
        }
    }

    pub fn set_autohides_scrollers(self, _autohides: bool) {}

    pub fn set_has_horizontal_scroller(self, has: bool) {
        if let Some(s) =
            self.try_downcast::<objc2_ui_kit::UIScrollView>()
        {
            s.setShowsHorizontalScrollIndicator(has);
        }
    }

    pub fn set_has_vertical_scroller(self, has: bool) {
        if let Some(s) =
            self.try_downcast::<objc2_ui_kit::UIScrollView>()
        {
            s.setShowsVerticalScrollIndicator(has);
        }
    }

    pub fn set_progress_displayed_when_stopped(self, _shown: bool) {}

    pub fn date_picker_value(self) -> crate::Date {
        if let Some(dp) =
            self.try_downcast::<objc2_ui_kit::UIDatePicker>()
        {
            let d = dp.date();
            return crate::Date::from_nsdate(&d);
        }
        crate::Date::now()
    }

    pub fn set_date_picker_value(self, d: crate::Date) {
        if let Some(dp) =
            self.try_downcast::<objc2_ui_kit::UIDatePicker>()
        {
            let current = dp.date();
            let current_secs = current.timeIntervalSince1970();
            if (current_secs - d.seconds_since_epoch).abs()
                > f64::EPSILON
            {
                dp.setDate(&d.to_nsdate());
            }
        }
    }

    pub fn stepper_value(self) -> f64 {
        if let Some(s) =
            self.try_downcast::<objc2_ui_kit::UIStepper>()
        {
            return s.value() as f64;
        }
        0.0
    }

    pub fn set_stepper_value(self, v: f64) {
        if let Some(s) =
            self.try_downcast::<objc2_ui_kit::UIStepper>()
        {
            if (s.value() as f64 - v).abs() > f64::EPSILON {
                s.setValue(v);
            }
        }
    }

    pub fn configure_stepper(
        self,
        min: f64,
        max: f64,
        increment: f64,
    ) {
        if let Some(s) =
            self.try_downcast::<objc2_ui_kit::UIStepper>()
        {
            s.setMinimumValue(min);
            s.setMaximumValue(max);
            s.setStepValue(increment);
        }
    }

    pub fn set_progress_value(self, v: f64) {
        if let Some(p) =
            self.try_downcast::<objc2_ui_kit::UIProgressView>()
        {
            p.setProgress(v as f32);
        }
    }

    pub fn set_progress_indeterminate(self, _indeterminate: bool) {}
    pub fn set_progress_max(self, _max: f64) {}

    pub fn on_text_view_change(
        self,
        cb: impl FnMut(String) + 'static,
    ) {
        crate::event::on_text_view_change(self, cb);
    }

    pub fn set_text_view_editable(self, editable: bool) {
        if let Some(tv) =
            self.try_downcast::<objc2_ui_kit::UITextView>()
        {
            if tv.isEditable() != editable {
                tv.setEditable(editable);
            }
        }
    }

    pub fn text_view_value(self) -> Option<String> {
        let tv =
            self.try_downcast::<objc2_ui_kit::UITextView>()?;
        Some(tv.text().to_string())
    }

    pub fn focus(self) -> bool {
        let Some(view) = self.try_ui_view() else { return false; };
        view.becomeFirstResponder()
    }

    pub fn blur(self) -> bool {
        let Some(view) = self.try_ui_view() else { return false; };
        view.resignFirstResponder()
    }

    pub fn set_image_view_path(self, path: &str) {
        use objc2_ui_kit::{UIImage, UIImageView};
        let Some(iv) = self.try_downcast::<UIImageView>() else {
            return;
        };
        if path.is_empty() {
            iv.setImage(None);
            return;
        }
        let path_str = NSString::from_str(path);
        let image =
            UIImage::imageWithContentsOfFile(&path_str);
        iv.setImage(image.as_deref());
        crate::layout::schedule_relayout(self);
    }

    pub fn set_image_view_bytes(self, bytes: Option<&[u8]>) {
        use objc2_ui_kit::{UIImage, UIImageView};
        use objc2_foundation::NSData;
        let Some(iv) = self.try_downcast::<UIImageView>() else {
            return;
        };
        let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
            iv.setImage(None);
            crate::layout::schedule_relayout(self);
            return;
        };
        let data = NSData::with_bytes(bytes);
        let image = UIImage::imageWithData(&data);
        iv.setImage(image.as_deref());
        crate::layout::schedule_relayout(self);
    }

    fn sf_symbol_image(name: &str) -> Option<objc2::rc::Retained<objc2_ui_kit::UIImage>> {
        use objc2_ui_kit::UIImage;
        if name.is_empty() {
            return None;
        }
        let ns_name = NSString::from_str(name);
        UIImage::systemImageNamed(&ns_name)
    }

    pub fn set_sf_symbol(self, name: &str) {
        let Some(view) = self.try_ui_view() else { return; };
        let image = Self::sf_symbol_image(name);
        if let Some(button) = downcast::<UIButton>(&view) {
            button.setImage_forState(
                image.as_deref(),
                objc2_ui_kit::UIControlState::Normal,
            );
            crate::layout::schedule_relayout(self);
            return;
        }
        if let Some(iv) = downcast::<objc2_ui_kit::UIImageView>(&view) {
            iv.setImage(image.as_deref());
            crate::layout::schedule_relayout(self);
        }
    }

    pub fn set_tint(self, color: Option<crate::Color>) {
        let Some(view) = self.try_ui_view() else { return; };
        unsafe {
            if let Some(c) = color {
                view.setTintColor(Some(&c.to_uicolor()));
            } else {
                view.setTintColor(None);
            }
        }
    }

    /// Build a text-label Node — a UILabel. Used by the renderer's
    /// `create_text_node`, which is the `Render` impl for `&str` /
    /// `String` / numerics.
    pub fn create_text(content: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_text_with(content, mtm)
    }

    pub fn create_text_with(
        content: &str,
        mtm: MainThreadMarker,
    ) -> Self {
        use objc2_ui_kit::UILabel;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let label = UILabel::initWithFrame(UILabel::alloc(mtm), frame);
        label.setText(Some(&NSString::from_str(content)));
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(label) };

        let mut style = crate::layout::Style::default();
        style.flex_shrink = 0.0;

        UikitElem::from_view(view, style, IosMeta::default())
    }

    /// Update the displayed string on a text-label Node. No-op if
    /// the backing view isn't a UILabel.
    pub fn set_text(self, content: &str) {
        let Some(view) = self.try_ui_view() else { return; };
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(&view) {
            label.setText(Some(&NSString::from_str(content)));
        }
        crate::layout::schedule_relayout(self);
    }

    /// Build a placeholder Node — a hidden, zero-sized UIView used
    /// by the renderer's control-flow primitives (`Render for ()`,
    /// tuple/iterator/keyed end-markers) as a stable mount anchor.
    pub fn create_placeholder() -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_placeholder_with(mtm)
    }

    pub fn create_placeholder_with(
        mtm: MainThreadMarker,
    ) -> Self {
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let view = UIView::initWithFrame(
            UIView::alloc(mtm),
            NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0)),
        );
        view.setHidden(true);

        let mut style = crate::layout::Style::default();
        style.position = crate::layout::Position::Absolute;
        style.size.width = crate::layout::Dimension::length(0.0);
        style.size.height = crate::layout::Dimension::length(0.0);

        UikitElem::from_view(view, style, IosMeta::default())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

pub(crate) fn downcast<T>(view: &UIView) -> Option<Retained<T>>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>().map(|r| r.retain())
}
