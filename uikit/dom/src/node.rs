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

use crate::layout::{IosMeta, LayoutHandle, NodeId, Style, TreeRef};
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_ui_kit::{UIButton, UIControl, UITextField, UIView};
use objc2_foundation::NSString;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, fmt, rc::Rc};

/// The core node wrapper — a thin handle into a `LayoutTree` arena.
///
/// `Node` is `Clone` (single Rc bump) and `Send + 'static` (via
/// [`SendWrapper`]). Touched only from the main thread.
///
/// Every Node clone shares one `Rc<NodeInner>`. When the last clone
/// of an OWNING Node drops, `NodeInner::Drop` calls
/// `tree.decref(id)`. The arena's removal rule (refcount=0 AND
/// parent=None) decides whether to actually free the entry.
///
/// For closure-capture patterns that need to refer back to a node
/// without forming reference cycles, use [`WeakNode`] / [`WeakElement`].
#[derive(Clone)]
pub struct Node {
    inner: SendWrapper<Rc<NodeInner>>,
}

pub(crate) struct NodeInner {
    tree: TreeRef,
    id: NodeId,
    /// Cached `Retained<UIView>` so `ui_view() -> &UIView` doesn't
    /// need an arena borrow. Adds one ObjC retain per `NodeInner`
    /// (the arena's `NodeData::view` is the other retain).
    view: Retained<UIView>,
    /// When true, `Drop` does NOT decref the arena entry.
    is_borrowed: bool,
}

impl Drop for NodeInner {
    fn drop(&mut self) {
        if !self.is_borrowed {
            self.tree.decref(self.id);
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr: *const UIView = &*self.inner.view;
        f.debug_struct("Node")
            .field("id", &self.inner.id)
            .field("ptr", &ptr)
            .field("borrowed", &self.inner.is_borrowed)
            .finish()
    }
}

impl AsRef<Node> for Node {
    fn as_ref(&self) -> &Node {
        self
    }
}

impl Node {
    /// Typed registration primitive: hand in a concrete UIView
    /// subclass, get back a `Node` owning a fresh arena entry. Used
    /// by every typed-builder construction path in `leptos_uikit`
    /// and by [`Node::create_text`] / [`Node::create_placeholder`].
    pub fn from_view<V>(
        tree: &TreeRef,
        view: Retained<V>,
        default_style: Style,
        default_meta: IosMeta,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        Self::create_in_tree(tree, view, default_style, default_meta)
    }

    /// Legacy alias for [`Self::from_view`].
    pub(crate) fn create_in_tree<V>(
        tree: &TreeRef,
        view: Retained<V>,
        default_style: Style,
        default_meta: IosMeta,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(view) };
        let view_for_arena = view.clone();
        let id = tree.new_leaf(
            default_style,
            SendWrapper::new(view_for_arena),
            default_meta,
            crate::event::IosNodeHandlers::default(),
        );
        // Wire the handlers' view back-ref so Drop can nil
        // setDelegate / removeAllTargets while the view is still alive.
        tree.with_handlers_mut(id, |h| h.attach_view(view.clone()));

        let inner = NodeInner {
            tree: tree.clone(),
            id,
            view,
            is_borrowed: false,
        };
        Node { inner: SendWrapper::new(Rc::new(inner)) }
    }

    /// Build a Node wrapping `view` with a pre-existing
    /// [`LayoutHandle`]. The resulting Node is **borrowed**: its
    /// `Drop` does NOT remove the arena entry.
    pub fn from_view_with_handle<V>(
        view: Retained<V>,
        handle: LayoutHandle,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(view) };
        let inner = NodeInner {
            tree: handle.tree,
            id: handle.node_id,
            view,
            is_borrowed: true,
        };
        Node { inner: SendWrapper::new(Rc::new(inner)) }
    }

    pub fn ui_view(&self) -> &UIView {
        &self.inner.view
    }

    pub fn ui_view_retained(&self) -> Retained<UIView> {
        self.inner.view.clone()
    }

    pub fn ptr_eq(&self, other: &Node) -> bool {
        let a: *const UIView = &*self.inner.view;
        let b: *const UIView = &*other.inner.view;
        a == b
    }

    /// Drop the resources owned by this node and detach it from its
    /// superview. Removes the arena entry eagerly (bypasses the
    /// refcount-still-positive reachability check).
    pub fn teardown(&self) {
        self.inner.tree.remove(self.inner.id);
        self.ui_view().removeFromSuperview();
    }

    // ---- Accessor surface ------------------------------------------

    /// Borrow the node's [`Style`] for read.
    pub fn with_style<R>(&self, f: impl FnOnce(&Style) -> R) -> R {
        let style = self.inner.tree.style(self.inner.id).unwrap_or_default();
        f(&style)
    }

    /// Mutate the node's [`Style`]. Pushed straight into the tree
    /// (which marks the node dirty).
    pub fn with_style_mut<R>(&self, f: impl FnOnce(&mut Style) -> R) -> R {
        let mut style = self.inner.tree.style(self.inner.id).unwrap_or_default();
        let r = f(&mut style);
        self.inner.tree.set_style(self.inner.id, style);
        r
    }

    /// Borrow the node's [`IosMeta`] for read.
    pub fn with_meta<R>(&self, f: impl FnOnce(&IosMeta) -> R) -> R {
        let meta = self.inner.tree.meta(self.inner.id).unwrap_or_default();
        f(&meta)
    }

    /// Mutate the node's [`IosMeta`]. Pushed back into the tree.
    pub fn with_meta_mut<R>(&self, f: impl FnOnce(&mut IosMeta) -> R) -> R {
        let mut meta = self.inner.tree.meta(self.inner.id).unwrap_or_default();
        let r = f(&mut meta);
        self.inner.tree.set_meta(self.inner.id, meta);
        r
    }

    /// Mutate this node's per-node handler set in the arena.
    pub fn with_handlers_mut<R>(
        &self,
        f: impl FnOnce(&mut crate::event::IosNodeHandlers) -> R,
    ) -> R {
        self.inner
            .tree
            .with_handlers_mut(self.inner.id, f)
            .expect("Node id must exist in arena")
    }

    /// Returns the `(TreeRef, NodeId)` pair. Always `Some` now that
    /// every Node has a tree from creation — the `Option` is kept for
    /// API stability.
    pub fn tree_id(&self) -> Option<(TreeRef, NodeId)> {
        Some((self.inner.tree.clone(), self.inner.id))
    }

    /// Cheap accessor for the LayoutHandle.
    pub fn mounted_handle(&self) -> Option<LayoutHandle> {
        Some(LayoutHandle {
            tree: self.inner.tree.clone(),
            node_id: self.inner.id,
        })
    }

    /// Test-only: number of strong refs to the inner Rc.
    #[doc(hidden)]
    pub fn handlers_rc_count_for_test(&self) -> usize {
        Rc::strong_count(&*self.inner)
    }
}

// ---------------------------------------------------------------------
// Node — typed-builder / renderer-protocol surface
// ---------------------------------------------------------------------

/// Backwards-compatibility alias. `Element` used to be a distinct
/// wrapper over `Node`; after the kind-discriminant + Text/Placeholder
/// unification, the wrapper had no remaining state. `Node` is now
/// the single user-facing type for every UIView-backed arena entry.
pub type Element = Node;

impl Node {
    /// Identity. Kept (along with [`Self::into_node`] /
    /// [`Self::from_node_unchecked`]) so the pre-unification call
    /// style (`el.as_node()`, `el.into_node()`,
    /// `Element::from_node_unchecked(n)`) keeps compiling.
    pub fn as_node(&self) -> &Node {
        self
    }

    /// Identity. See [`Self::as_node`].
    pub fn into_node(self) -> Node {
        self
    }

    /// Identity. See [`Self::as_node`].
    pub fn from_node_unchecked(node: Node) -> Node {
        node
    }

    /// Generic UIView container (default style). Used by
    /// `<view>` / `<stack>` builders and by `RootViewController`
    /// for the content root.
    pub fn create_container(tree: &TreeRef) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_container_with(tree, mtm)
    }

    pub fn create_container_with(tree: &TreeRef, mtm: MainThreadMarker) -> Self {
        use objc2_foundation::{NSPoint, NSRect, NSSize};
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let view: Retained<UIView> = UIView::initWithFrame(UIView::alloc(mtm), frame);
        Node::from_view(tree, view, Style::default(), IosMeta::default())
    }


    /// The UIView that *actually* parents this element's children.
    pub fn subview_parent(&self) -> Retained<UIView> {
        let direct = self.ui_view();
        let routes_to_doc = self.with_meta(|m| m.is_scroll_view);
        if routes_to_doc {
            if let Some(scroll) =
                downcast::<objc2_ui_kit::UIScrollView>(direct)
            {
                let subs = scroll.subviews();
                if subs.count() > 0 {
                    return subs.objectAtIndex(0);
                }
            }
        }
        direct.into()
    }

    pub fn insert_node(&self, child: &Node, marker: Option<&Node>) {
        let parent_retained = self.subview_parent();
        let parent: &UIView = &parent_retained;
        let child_view = child.ui_view();

        match marker {
            None => {
                parent.addSubview(child_view);
                crate::layout::attach_child(self.as_node(), child);
            }
            Some(marker) => {
                let marker_view = marker.ui_view();
                parent.insertSubview_belowSubview(child_view, marker_view);
                let subviews = parent.subviews();
                let child_ptr: *const UIView = child_view;
                let mut child_index = subviews.len();
                for (i, sv) in subviews.iter().enumerate() {
                    let sv_ptr: *const UIView = &*sv;
                    if sv_ptr == child_ptr {
                        child_index = i;
                        break;
                    }
                }
                crate::layout::insert_child_at(
                    self.as_node(),
                    child,
                    child_index,
                );
            }
        }
    }

    pub fn remove_child(&self, child: &Node) -> Option<Node> {
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
        crate::layout::detach_child(self.as_node(), child);
        Some(child.clone())
    }

    pub fn clear_children(&self) {
        let parent_retained = self.subview_parent();
        let parent: &UIView = &parent_retained;
        let subs = parent.subviews();
        for sv in subs.iter() {
            sv.removeFromSuperview();
        }
    }

    /// Set the title on a UIButton (Normal state) or the text on a
    /// UILabel. No-op on other classes.
    pub fn set_title(&self, value: &str) {
        let view = self.ui_view();
        let mut changed = false;
        if let Some(button) = downcast::<UIButton>(view) {
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
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
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
    pub fn set_value(&self, value: &str) {
        let view = self.ui_view();
        let mut changed = false;
        if let Some(field) = downcast::<UITextField>(view) {
            let current = field.text().map(|s| s.to_string()).unwrap_or_default();
            if current != value {
                field.setText(Some(&NSString::from_str(value)));
                changed = true;
            }
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
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
    pub fn set_placeholder(&self, value: &str) {
        let view = self.ui_view();
        if let Some(field) = downcast::<UITextField>(view) {
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
    pub fn set_hidden(&self, value: bool) {
        let view = self.ui_view();
        if view.isHidden() != value {
            view.setHidden(value);
        }
    }

    /// Toggle user-interaction / enabled state. Sets
    /// `isUserInteractionEnabled` on the UIView; for UIControl
    /// subclasses, also sets `isEnabled`.
    pub fn set_enabled(&self, value: bool) {
        let view = self.ui_view();
        if view.isUserInteractionEnabled() != value {
            view.setUserInteractionEnabled(value);
        }
        if let Some(control) = downcast::<UIControl>(view) {
            if control.isEnabled() != value {
                control.setEnabled(value);
            }
        }
    }

    /// Set the on/off state on a UISwitch (animated). No-op on
    /// other classes.
    pub fn set_checked(&self, value: bool) {
        let view = self.ui_view();
        if let Some(sw) = downcast::<objc2_ui_kit::UISwitch>(view) {
            if sw.isOn() != value {
                sw.setOn_animated(value, true);
            }
        }
    }

    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        let view = self.ui_view();
        if downcast::<UIControl>(view).is_some() {
            crate::event::on_control_action(self, cb);
        } else {
            crate::event::on_tap_gesture(self, cb);
        }
    }

    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        crate::event::on_control_action(self, cb);
    }

    pub fn on_value_change(&self, mut cb: impl FnMut() + Send + 'static) {
        if downcast::<UITextField>(self.ui_view()).is_some() {
            crate::event::on_text_field_change(self, move |_| cb());
            return;
        }
        crate::event::on_control_action(self, cb);
    }

    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_field_change(self, cb);
    }

    pub fn on_text_end_editing(&self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_field_end_editing(self, cb);
    }

    pub fn on_text_focus(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_focus(self, cb);
    }

    pub fn on_text_blur(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_blur(self, cb);
    }

    pub fn on_text_keydown(
        &self,
        _cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        // Deferred: UIKeyCommand + pressesBegan:
    }

    pub fn on_text_keyup(
        &self,
        _cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
    }

    pub fn checked(&self) -> bool {
        if let Some(sw) = downcast::<objc2_ui_kit::UISwitch>(self.ui_view()) {
            return sw.isOn();
        }
        false
    }

    pub fn double_value(&self) -> f64 {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            return sl.value() as f64;
        }
        0.0
    }

    pub fn set_double_value(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            let current = sl.value() as f64;
            if (current - v).abs() > f64::EPSILON {
                sl.setValue(v as f32);
            }
        }
    }

    pub fn set_slider_min(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            sl.setMinimumValue(v as f32);
        }
    }

    pub fn set_slider_max(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            sl.setMaximumValue(v as f32);
        }
    }

    pub fn set_segmented_items(&self, items: &[String]) {
        let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
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

    pub fn segmented_selection(&self) -> isize {
        if let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
        {
            return sc.selectedSegmentIndex();
        }
        -1
    }

    pub fn set_segmented_selection(&self, idx: isize) {
        if let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
        {
            if sc.selectedSegmentIndex() != idx {
                sc.setSelectedSegmentIndex(idx);
            }
        }
    }

    pub fn set_popup_items(
        &self,
        items: &[String],
        selected_idx: usize,
        on_select: impl FnMut(usize) + 'static,
    ) {
        use objc2_ui_kit::{UIAction, UIMenu, UIMenuElement, UIMenuElementState};
        let Some(button) = downcast::<UIButton>(self.ui_view()) else {
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

    pub fn set_popup_selection(&self, items: &[String], idx: usize) {
        let Some(button) = downcast::<UIButton>(self.ui_view()) else {
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

    pub fn set_color_well_value(&self, color: crate::Color) {
        use objc2_ui_kit::UIColorWell;
        let Some(cw) = downcast::<UIColorWell>(self.ui_view()) else {
            return;
        };
        cw.setSelectedColor(Some(&color.to_uicolor()));
    }

    pub fn color_well_value(&self) -> Option<crate::Color> {
        use objc2_ui_kit::UIColorWell;
        let cw = downcast::<UIColorWell>(self.ui_view())?;
        let c = cw.selectedColor()?;
        crate::Color::from_uicolor(&c)
    }

    pub fn on_color_change(
        &self,
        mut cb: impl FnMut(crate::Color) + 'static,
    ) {
        use objc2_ui_kit::UIColorWell;
        let Some(cw) = downcast::<UIColorWell>(self.ui_view()) else {
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

    pub fn set_alpha(&self, alpha: f64) {
        let v = self.ui_view();
        let clamped = alpha.clamp(0.0, 1.0);
        if (v.alpha() - clamped).abs() > f64::EPSILON {
            v.setAlpha(clamped);
        }
    }

    pub fn set_background_color(&self, color: Option<crate::Color>) {
        let v = self.ui_view();
        match color {
            Some(c) => v.setBackgroundColor(Some(&c.to_uicolor())),
            None => v.setBackgroundColor(None),
        }
    }

    pub fn set_corner_radius(&self, radius: f64) {
        let layer = self.ui_view().layer();
        if (layer.cornerRadius() - radius).abs() > f64::EPSILON {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(radius > 0.0);
        }
    }

    pub fn set_border_width(&self, width: f64) {
        let layer = self.ui_view().layer();
        if (layer.borderWidth() - width).abs() > f64::EPSILON {
            layer.setBorderWidth(width);
        }
    }

    pub fn set_border_color(&self, color: Option<crate::Color>) {
        let layer = self.ui_view().layer();
        match color {
            Some(c) => {
                let cg = unsafe { c.to_uicolor().CGColor() };
                layer.setBorderColor(Some(&cg));
            }
            None => layer.setBorderColor(None),
        }
    }

    pub fn set_text_color(&self, color: crate::Color) {
        let view = self.ui_view();
        let uicolor = color.to_uicolor();

        if let Some(field) = downcast::<UITextField>(view) {
            field.setTextColor(Some(&uicolor));
            return;
        }
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
            unsafe { label.setTextColor(Some(&uicolor)) };
            return;
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
            tv.setTextColor(Some(&uicolor));
        }
    }

    pub fn set_text_alignment(&self, alignment: crate::TextAlignment) {
        let view = self.ui_view();

        if let Some(field) = downcast::<UITextField>(view) {
            field.setTextAlignment(alignment.0);
            return;
        }
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
            label.setTextAlignment(alignment.0);
            return;
        }
        if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
            tv.setTextAlignment(alignment.0);
        }
    }

    pub fn set_font_size(&self, points: f64) {
        use objc2_ui_kit::UIFont;
        let font = UIFont::systemFontOfSize(points);

        let view = self.ui_view();
        let mut applied = false;
        if let Some(field) = downcast::<UITextField>(view) {
            field.setFont(Some(&font));
            applied = true;
        } else if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
            unsafe { label.setFont(Some(&font)) };
            applied = true;
        } else if let Some(button) = downcast::<UIButton>(view) {
            if let Some(title_label) = button.titleLabel() {
                unsafe { title_label.setFont(Some(&font)) };
                applied = true;
            }
        } else if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
            tv.setFont(Some(&font));
            applied = true;
        }
        if applied {
            crate::layout::schedule_relayout(self);
        }
    }

    pub fn set_text_field_bordered(&self, bordered: bool) {
        if let Some(f) = downcast::<UITextField>(self.ui_view()) {
            use objc2_ui_kit::UITextBorderStyle;
            f.setBorderStyle(if bordered {
                UITextBorderStyle::RoundedRect
            } else {
                UITextBorderStyle::None
            });
        }
    }

    pub fn set_text_field_bezeled(&self, bezeled: bool) {
        self.set_text_field_bordered(bezeled);
    }

    pub fn set_slider_vertical(&self, _vertical: bool) {}
    pub fn set_slider_tick_marks(&self, _count: usize) {}
    pub fn set_slider_snaps_to_ticks(&self, _snaps: bool) {}

    pub fn set_date_picker_style(&self, style: crate::DatePickerStyle) {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            dp.setPreferredDatePickerStyle(style.0);
        }
    }

    pub fn set_date_picker_min(&self, d: Option<crate::Date>) {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMinimumDate(nd.as_deref());
        }
    }

    pub fn set_date_picker_max(&self, d: Option<crate::Date>) {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMaximumDate(nd.as_deref());
        }
    }

    pub fn set_autohides_scrollers(&self, _autohides: bool) {}

    pub fn set_has_horizontal_scroller(&self, has: bool) {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIScrollView>(self.ui_view())
        {
            s.setShowsHorizontalScrollIndicator(has);
        }
    }

    pub fn set_has_vertical_scroller(&self, has: bool) {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIScrollView>(self.ui_view())
        {
            s.setShowsVerticalScrollIndicator(has);
        }
    }

    pub fn set_progress_displayed_when_stopped(&self, _shown: bool) {}

    pub fn date_picker_value(&self) -> crate::Date {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            let d = dp.date();
            return crate::Date::from_nsdate(&d);
        }
        crate::Date::now()
    }

    pub fn set_date_picker_value(&self, d: crate::Date) {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
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

    pub fn stepper_value(&self) -> f64 {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIStepper>(self.ui_view())
        {
            return s.value() as f64;
        }
        0.0
    }

    pub fn set_stepper_value(&self, v: f64) {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIStepper>(self.ui_view())
        {
            if (s.value() as f64 - v).abs() > f64::EPSILON {
                s.setValue(v);
            }
        }
    }

    pub fn configure_stepper(
        &self,
        min: f64,
        max: f64,
        increment: f64,
    ) {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIStepper>(self.ui_view())
        {
            s.setMinimumValue(min);
            s.setMaximumValue(max);
            s.setStepValue(increment);
        }
    }

    pub fn set_progress_value(&self, v: f64) {
        if let Some(p) =
            downcast::<objc2_ui_kit::UIProgressView>(self.ui_view())
        {
            p.setProgress(v as f32);
        }
    }

    pub fn set_progress_indeterminate(&self, _indeterminate: bool) {}
    pub fn set_progress_max(&self, _max: f64) {}

    pub fn on_text_view_change(
        &self,
        cb: impl FnMut(String) + 'static,
    ) {
        crate::event::on_text_view_change(self, cb);
    }

    pub fn set_text_view_editable(&self, editable: bool) {
        if let Some(tv) =
            downcast::<objc2_ui_kit::UITextView>(self.ui_view())
        {
            if tv.isEditable() != editable {
                tv.setEditable(editable);
            }
        }
    }

    pub fn text_view_value(&self) -> Option<String> {
        let tv =
            downcast::<objc2_ui_kit::UITextView>(self.ui_view())?;
        Some(tv.text().to_string())
    }

    pub fn focus(&self) -> bool {
        let view = self.ui_view();
        view.becomeFirstResponder()
    }

    pub fn blur(&self) -> bool {
        let view = self.ui_view();
        view.resignFirstResponder()
    }

    pub fn set_image_view_path(&self, path: &str) {
        use objc2_ui_kit::{UIImage, UIImageView};
        let Some(iv) = downcast::<UIImageView>(self.ui_view()) else {
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

    pub fn set_image_view_bytes(&self, bytes: Option<&[u8]>) {
        use objc2_ui_kit::{UIImage, UIImageView};
        use objc2_foundation::NSData;
        let Some(iv) = downcast::<UIImageView>(self.ui_view()) else {
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

    pub fn set_sf_symbol(&self, name: &str) {
        let view = self.ui_view();
        let image = Self::sf_symbol_image(name);
        if let Some(button) = downcast::<UIButton>(view) {
            button.setImage_forState(
                image.as_deref(),
                objc2_ui_kit::UIControlState::Normal,
            );
            crate::layout::schedule_relayout(self);
            return;
        }
        if let Some(iv) = downcast::<objc2_ui_kit::UIImageView>(view) {
            iv.setImage(image.as_deref());
            crate::layout::schedule_relayout(self);
        }
    }

    pub fn set_tint(&self, color: Option<crate::Color>) {
        let view = self.ui_view();
        unsafe {
            if let Some(c) = color {
                view.setTintColor(Some(&c.to_uicolor()));
            } else {
                view.setTintColor(None);
            }
        }
    }

}

// ---------------------------------------------------------------------
// Node: text-label & placeholder constructors
// ---------------------------------------------------------------------

impl Node {
    /// Build a text-label Node — a UILabel. Used by the renderer's
    /// `create_text_node`, which is the `Render` impl for `&str` /
    /// `String` / numerics.
    pub fn create_text(tree: &TreeRef, content: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_text_with(tree, content, mtm)
    }

    pub fn create_text_with(
        tree: &TreeRef,
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

        Node::from_view(tree, view, style, IosMeta::default())
    }

    /// Update the displayed string on a text-label Node. No-op if
    /// the backing view isn't a UILabel.
    pub fn set_text(&self, content: &str) {
        let view = self.ui_view();
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
            label.setText(Some(&NSString::from_str(content)));
        }
        crate::layout::schedule_relayout(self);
    }

    /// Build a placeholder Node — a hidden, zero-sized UIView used
    /// by the renderer's control-flow primitives (`Render for ()`,
    /// tuple/iterator/keyed end-markers) as a stable mount anchor.
    pub fn create_placeholder(tree: &TreeRef) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_placeholder_with(tree, mtm)
    }

    pub fn create_placeholder_with(
        tree: &TreeRef,
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

        Node::from_view(tree, view, style, IosMeta::default())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

pub(crate) fn downcast<T>(view: &UIView) -> Option<&T>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>()
}

// ---------------------------------------------------------------------
// Weak handles — non-owning references for cycle-safe closure capture
// ---------------------------------------------------------------------
//
// See `cocoa/dom/src/node.rs` for the longer rationale. The same
// Element-capture cycle risk applies on iOS (UIControl target/action
// + UITextView delegate); `WeakElement` is the safe alternative.

/// Non-owning weak reference to a `Node`.
#[derive(Clone)]
pub struct WeakNode {
    inner: SendWrapper<std::rc::Weak<NodeInner>>,
}

impl Node {
    pub fn downgrade(&self) -> WeakNode {
        WeakNode {
            inner: SendWrapper::new(Rc::downgrade(&*self.inner)),
        }
    }
}

impl WeakNode {
    pub fn upgrade(&self) -> Option<Node> {
        self.inner
            .upgrade()
            .map(|rc| Node { inner: SendWrapper::new(rc) })
    }

    pub fn is_alive(&self) -> bool {
        self.inner.strong_count() > 0
    }
}

impl fmt::Debug for WeakNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WeakNode")
            .field("alive", &self.is_alive())
            .finish()
    }
}

/// Backwards-compat alias for [`WeakNode`] — see [`Element`].
pub type WeakElement = WeakNode;

impl Node {
    /// Convenience alias for [`Node::downgrade`] (historical name).
    pub fn weak(&self) -> WeakNode {
        self.downgrade()
    }
}
