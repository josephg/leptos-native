//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `Retained<UIView>`.
//!
//! Each Node carries a *shared* layout slot (Rc'd among clones)
//! holding its current Taffy [`Style`] plus an `Option<LayoutHandle>`.
//! The handle is `None` until the node is mounted into a tree (a
//! [`Window`](crate::window)'s `TaffyTree`); style mutations made before
//! that point are buffered locally and pushed to the tree at
//! registration time. See `crate::layout` for the registration helpers.
//!
//! See the crate-level docs for the threading contract.

use crate::layout::{Dimension, LayoutHandle, NodeLayout, Style};
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_ui_kit::{
    UIButton, UIControl, UITextField, UITextInputTraits, UIView,
};
use objc2_foundation::NSString;
use send_wrapper::SendWrapper;
use std::{cell::RefCell, fmt, rc::Rc};

/// Compile-time-checked attribute identifiers, split by value type.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StringAttr {
    Title,
    Value,
    Placeholder,
}

impl StringAttr {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "title" => Self::Title,
            "value" => Self::Value,
            "placeholder" => Self::Placeholder,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Value => "value",
            Self::Placeholder => "placeholder",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BoolAttr {
    Enabled,
    Hidden,
    Checked,
}

impl BoolAttr {
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "enabled" => Self::Enabled,
            "hidden" => Self::Hidden,
            "checked" => Self::Checked,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Hidden => "hidden",
            Self::Checked => "checked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Element,
    Text,
    Placeholder,
}

#[derive(Clone)]
pub struct Node {
    view: SendWrapper<Retained<UIView>>,
    layout: SendWrapper<Rc<RefCell<NodeLayout>>>,
    kind: NodeKind,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr: *const UIView = &**self.view;
        f.debug_struct("Node")
            .field("kind", &self.kind)
            .field("ptr", &ptr)
            .field("registered", &self.layout.borrow().handle.is_some())
            .finish()
    }
}

impl AsRef<Node> for Element {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl AsRef<Node> for Text {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl AsRef<Node> for Placeholder {
    fn as_ref(&self) -> &Node {
        &self.node
    }
}

impl Node {
    pub fn from_view<V>(
        view: Retained<V>,
        kind: NodeKind,
        default_style: Style,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(view) };
        Node {
            view: SendWrapper::new(view),
            layout: SendWrapper::new(Rc::new(RefCell::new(NodeLayout::new(
                default_style,
            )))),
            kind,
        }
    }

    pub fn from_view_with_handle<V>(
        view: Retained<V>,
        kind: NodeKind,
        handle: LayoutHandle,
    ) -> Self
    where
        V: AsRef<UIView> + Message,
    {
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(view) };
        let mut layout = NodeLayout::new(Style::default());
        layout.handle = Some(handle);
        Node {
            view: SendWrapper::new(view),
            layout: SendWrapper::new(Rc::new(RefCell::new(layout))),
            kind,
        }
    }

    pub fn ui_view(&self) -> &UIView {
        &self.view
    }

    pub fn into_ui_view(self) -> Retained<UIView> {
        self.view.take()
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    pub fn layout_slot(&self) -> &RefCell<NodeLayout> {
        &**self.layout
    }

    pub fn ptr_eq(&self, other: &Node) -> bool {
        let a: *const UIView = &**self.view;
        let b: *const UIView = &**other.view;
        a == b
    }

    pub fn teardown(&self) {
        crate::event::drop_handlers_for(self.ui_view());
        // For UIScrollView-backed `<scroll_view>`, the content UIView
        // holds its own handler store entries. Walk one level deeper
        // so they don't leak.
        let view = self.ui_view();
        if let Some(scroll) = downcast::<objc2_ui_kit::UIScrollView>(view) {
            let subs = scroll.subviews();
            if subs.count() > 0 {
                let first = subs.objectAtIndex(0);
                crate::event::drop_handlers_for(&first);
            }
        }
        crate::layout::drop_node(self);
        self.ui_view().removeFromSuperview();
    }
}

// ---------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Element {
    node: Node,
}

impl Element {
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Element,
            "Element::from_node_unchecked called with a non-Element node"
        );
        Element { node }
    }

    pub fn create(tag: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_with(tag, mtm)
    }

    pub fn create_with(tag: &str, mtm: MainThreadMarker) -> Self {
        use crate::layout::{FlexDirection, Style};
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));

        let (view, default_style): (Retained<UIView>, Style) = match tag {
            "button" => {
                // UIButton::buttonWithType: UIButtonTypeSystem is the
                // standard iOS push button (blue tint, no bezel, ~44pt
                // default height). Title and target/action are set
                // later via attributes / on_click.
                let b = UIButton::buttonWithType(
                    objc2_ui_kit::UIButtonType::System,
                    mtm,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(b) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "switch" => {
                // UISwitch: on/off toggle, fixed intrinsic size (51×31).
                // Use `bind:checked` and `on:change` (via target/action
                // for UIControlEventValueChanged).
                use objc2_ui_kit::UISwitch;
                let sw = UISwitch::initWithFrame(
                    UISwitch::alloc(mtm),
                    frame,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(sw) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "label" => {
                // UILabel — the natural iOS text display. Not editable.
                use objc2_ui_kit::UILabel;
                let l = UILabel::initWithFrame(
                    UILabel::alloc(mtm),
                    frame,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(l) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "text_field" => {
                let tf = UITextField::initWithFrame(
                    UITextField::alloc(mtm),
                    frame,
                );
                tf.setBorderStyle(objc2_ui_kit::UITextBorderStyle::RoundedRect);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(tf) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "secure_text_field" => {
                // iOS doesn't have a separate secure text field class.
                // UITextField with secureTextEntry = YES.
                let tf = UITextField::initWithFrame(
                    UITextField::alloc(mtm),
                    frame,
                );
                tf.setSecureTextEntry(true);
                tf.setBorderStyle(objc2_ui_kit::UITextBorderStyle::RoundedRect);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(tf) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "slider" => {
                use objc2_ui_kit::UISlider;
                let sl = UISlider::initWithFrame(
                    UISlider::alloc(mtm),
                    frame,
                );
                sl.setContinuous(true);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(sl) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "date_picker" => {
                use objc2_ui_kit::UIDatePicker;
                let dp = UIDatePicker::initWithFrame(
                    UIDatePicker::alloc(mtm),
                    frame,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(dp) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "stepper" => {
                use objc2_ui_kit::UIStepper;
                let st = UIStepper::initWithFrame(
                    UIStepper::alloc(mtm),
                    frame,
                );
                st.setAutorepeat(true);
                st.setContinuous(true);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(st) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "progress_indicator" => {
                use objc2_ui_kit::UIProgressView;
                let pv = UIProgressView::initWithProgressViewStyle(
                    UIProgressView::alloc(mtm),
                    objc2_ui_kit::UIProgressViewStyle::Default,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(pv) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "image_view" => {
                use objc2_ui_kit::UIImageView;
                let iv = UIImageView::initWithFrame(
                    UIImageView::alloc(mtm),
                    frame,
                );
                iv.setContentMode(objc2_ui_kit::UIViewContentMode::ScaleAspectFit);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(iv) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "segmented_control" => {
                use objc2_ui_kit::UISegmentedControl;
                let sc = UISegmentedControl::initWithFrame(
                    UISegmentedControl::alloc(mtm),
                    frame,
                );
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(sc) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "scroll_view" => {
                // UIScrollView with a content UIView child.
                // Children added via `insert_node` are routed to the
                // content view via `Element::subview_parent`.
                use objc2_ui_kit::UIScrollView;
                let scroll = UIScrollView::initWithFrame(
                    UIScrollView::alloc(mtm),
                    frame,
                );
                scroll.setShowsVerticalScrollIndicator(true);
                scroll.setShowsHorizontalScrollIndicator(false);

                // Content view — all children go here.
                let content = UIView::initWithFrame(
                    UIView::alloc(mtm),
                    frame,
                );
                scroll.addSubview(&content);

                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(scroll) };
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Column;
                s.flex_basis = Dimension::length(0.0);
                s.min_size.height = Dimension::length(0.0);
                s.overflow = taffy::Point {
                    x: taffy::Overflow::Hidden,
                    y: taffy::Overflow::Hidden,
                };
                (v, s)
            }
            "text_view" => {
                // UITextView IS a UIScrollView subclass — it handles
                // its own scrolling natively. No wrapper needed.
                use objc2_ui_kit::UITextView;
                let tv = UITextView::initWithFrame(
                    UITextView::alloc(mtm),
                    frame,
                );
                tv.setEditable(true);
                tv.setSelectable(true);
                let v: Retained<UIView> = unsafe { Retained::cast_unchecked(tv) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "stack_view" | "vstack" | "view" => {
                let v: Retained<UIView> = UIView::initWithFrame(
                    UIView::alloc(mtm),
                    frame,
                );
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Column;
                (v, s)
            }
            "hstack" => {
                let v: Retained<UIView> = UIView::initWithFrame(
                    UIView::alloc(mtm),
                    frame,
                );
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Row;
                (v, s)
            }
            "grid" => {
                // 2-D grid container backed by Taffy's grid algorithm.
                // Template tracks / gap / placement attrs are applied
                // by the higher-level builder; this just establishes
                // the container's `display: grid`.
                let v: Retained<UIView> = UIView::initWithFrame(
                    UIView::alloc(mtm),
                    frame,
                );
                let mut s = Style::default();
                s.display = crate::layout::Display::Grid;
                (v, s)
            }
            // Unknown tags → generic UIView container.
            _ => {
                let v: Retained<UIView> = UIView::initWithFrame(
                    UIView::alloc(mtm),
                    frame,
                );
                (v, Style::default())
            }
        };

        let node = Node::from_view(view, NodeKind::Element, default_style);
        if tag == "scroll_view" {
            node.layout_slot().borrow_mut().meta.is_scroll_view = true;
        }

        Element { node }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    pub fn ui_view(&self) -> &UIView {
        self.node.ui_view()
    }

    /// The UIView that *actually* parents this element's children.
    /// For `<scroll_view>` this is the content UIView (first subview
    /// of the UIScrollView). For everything else it's self.
    pub fn subview_parent(&self) -> Retained<UIView> {
        let direct = self.ui_view();
        let routes_to_doc =
            self.node.layout_slot().borrow().meta.is_scroll_view;
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

    /// Detach every UIView child without touching Taffy or handler
    /// state. Tachys parents own their child `Node`s and call
    /// `teardown` on them via `Mountable::unmount`, which is what
    /// drops Taffy entries and event-target retentions — this
    /// method only handles the UIView side. Mirrors
    /// `cocoa_dom::Element::clear_children`.
    pub fn clear_children(&self) {
        let parent_retained = self.subview_parent();
        let parent: &UIView = &parent_retained;
        // subviews returns a copy, so iterating + removing is safe.
        let subs = parent.subviews();
        for sv in subs.iter() {
            sv.removeFromSuperview();
        }
    }

    pub fn set_attribute(&self, name: &str, value: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.set_string_attribute(attr, value);
        }
    }

    pub fn set_string_attribute(&self, attr: StringAttr, value: &str) {
        let view = self.ui_view();
        let mut content_changed = false;
        match attr {
            StringAttr::Title => {
                if let Some(button) = downcast::<UIButton>(view) {
                    let current = button
                        .titleForState(objc2_ui_kit::UIControlState::Normal)
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    if current != value {
                        let s = NSString::from_str(value);
                        button.setTitle_forState(
                            Some(&s),
                            objc2_ui_kit::UIControlState::Normal,
                        );
                        content_changed = true;
                    }
                }
                // Also set text on UILabel (used by label tag)
                if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
                    let current = label.text().map(|s| s.to_string()).unwrap_or_default();
                    if current != value {
                        let s = NSString::from_str(value);
                        label.setText(Some(&s));
                        content_changed = true;
                    }
                }
            }
            StringAttr::Value => {
                if let Some(field) = downcast::<UITextField>(view) {
                    let current = field.text().map(|s| s.to_string()).unwrap_or_default();
                    if current != value {
                        let s = NSString::from_str(value);
                        field.setText(Some(&s));
                        content_changed = true;
                    }
                }
                if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
                    let current = tv.text().to_string();
                    if current != value {
                        tv.setText(Some(&NSString::from_str(value)));
                        content_changed = true;
                    }
                }
            }
            StringAttr::Placeholder => {
                if let Some(field) = downcast::<UITextField>(view) {
                    let current: String = field
                        .placeholder()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    if current != value {
                        let s = NSString::from_str(value);
                        field.setPlaceholder(Some(&s));
                        content_changed = true;
                    }
                }
            }
        }
        if content_changed {
            crate::layout::schedule_relayout(&self.node);
        }
    }

    pub fn set_bool_attribute(&self, attr: BoolAttr, value: bool) {
        let view = self.ui_view();
        match attr {
            BoolAttr::Hidden => {
                if view.isHidden() != value {
                    view.setHidden(value);
                }
            }
            BoolAttr::Enabled => {
                // On iOS, UIView doesn't have an `enabled` property.
                // UIControl does. Set userInteractionEnabled on all
                // views, and additionally set enabled on UIControl
                // subclasses.
                if view.isUserInteractionEnabled() != value {
                    view.setUserInteractionEnabled(value);
                }
                if let Some(control) = downcast::<UIControl>(view) {
                    if control.isEnabled() != value {
                        control.setEnabled(value);
                    }
                }
            }
            BoolAttr::Checked => {
                // UISwitch: setOn:animated:
                if let Some(sw) = downcast::<objc2_ui_kit::UISwitch>(view) {
                    if sw.isOn() != value {
                        sw.setOn_animated(value, true);
                    }
                }
            }
        }
    }

    /// Wire a tap / value-change handler.
    ///
    /// For any `UIControl` this routes through the standard
    /// target/action machinery — `on_control_action` picks the
    /// right `UIControlEvents` mask based on the concrete control
    /// (`TouchUpInside` for `UIButton`, `ValueChanged` for
    /// `UISwitch` / `UISlider` / `UISegmentedControl` /
    /// `UIDatePicker` / `UIStepper`). For everything else
    /// (`UILabel`, `UIImageView`, container `UIView`s) we install a
    /// `UITapGestureRecognizer` so plain views can be tapped too.
    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        let view = self.ui_view();
        if let Some(c) = downcast::<UIControl>(view) {
            crate::event::on_control_action(c, cb);
        } else {
            crate::event::on_tap_gesture(view, cb);
        }
    }

    /// Wire a callback for UIControl value changes (sliders, switches,
    /// segmented controls, date pickers, steppers).
    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        if let Some(c) = downcast::<UIControl>(self.ui_view()) {
            crate::event::on_control_action(c, cb);
        }
    }

    /// Unit-payload "value changed" hook. Text fields fan to
    /// editingChanged (every keystroke). UISwitch / UISlider /
    /// UIStepper / UISegmentedControl / UIDatePicker fan to
    /// ValueChanged. Other views are no-op.
    pub fn on_value_change(&self, mut cb: impl FnMut() + Send + 'static) {
        if let Some(field) = downcast::<UITextField>(self.ui_view()) {
            crate::event::on_text_field_change(field, move |_| cb());
            return;
        }
        if let Some(c) = downcast::<UIControl>(self.ui_view()) {
            crate::event::on_control_action(c, cb);
        }
    }

    /// Wire a callback that fires on every text change (keystroke /
    /// paste / clear). Uses `editingChanged` UIControl event on
    /// UITextField. No-op on non-UITextField.
    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        if let Some(field) = downcast::<UITextField>(self.ui_view()) {
            crate::event::on_text_field_change(field, cb);
        }
    }

    /// Wire a callback that fires when editing ends (Return key,
    /// focus loss). Uses `editingDidEnd` UIControl event.
    pub fn on_text_end_editing(&self, cb: impl FnMut(String) + 'static) {
        if let Some(field) = downcast::<UITextField>(self.ui_view()) {
            crate::event::on_text_field_end_editing(field, cb);
        }
    }

    /// Wire a callback that fires when the text field gains focus.
    pub fn on_text_focus(&self, cb: impl FnMut() + 'static) {
        if let Some(field) = downcast::<UITextField>(self.ui_view()) {
            crate::event::on_text_field_focus(field, cb);
        }
    }

    /// Wire a callback that fires when the text field loses focus.
    pub fn on_text_blur(&self, cb: impl FnMut() + 'static) {
        if let Some(field) = downcast::<UITextField>(self.ui_view()) {
            crate::event::on_text_field_blur(field, cb);
        }
    }

    /// Wire a key event observer on a text field (hardware keyboard).
    /// Stub for v1 — hardware keyboard events are deferred.
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
        // Deferred: UIKeyCommand + pressesBegan:
    }

    /// Read the on/off state of a UISwitch.
    pub fn checked(&self) -> bool {
        if let Some(sw) = downcast::<objc2_ui_kit::UISwitch>(self.ui_view()) {
            return sw.isOn();
        }
        false
    }

    /// Read the current value of a UISlider.
    pub fn double_value(&self) -> f64 {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            return sl.value() as f64;
        }
        0.0
    }

    /// Set the value on a UISlider. Diffs to avoid redundant redraws.
    pub fn set_double_value(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            let current = sl.value() as f64;
            if (current - v).abs() > f64::EPSILON {
                sl.setValue(v as f32);
            }
        }
    }

    /// Slider min.
    pub fn set_slider_min(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            sl.setMinimumValue(v as f32);
        }
    }

    /// Slider max.
    pub fn set_slider_max(&self, v: f64) {
        if let Some(sl) = downcast::<objc2_ui_kit::UISlider>(self.ui_view()) {
            sl.setMaximumValue(v as f32);
        }
    }

    /// Set segment labels on a UISegmentedControl.
    pub fn set_segmented_items(&self, items: &[String]) {
        let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
        else {
            return;
        };
        // Remove all existing segments and re-add.
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

    /// Currently-selected segment index (-1 if none).
    pub fn segmented_selection(&self) -> isize {
        if let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
        {
            return sc.selectedSegmentIndex();
        }
        -1
    }

    /// Programmatically select a segment.
    pub fn set_segmented_selection(&self, idx: isize) {
        if let Some(sc) =
            downcast::<objc2_ui_kit::UISegmentedControl>(self.ui_view())
        {
            if sc.selectedSegmentIndex() != idx {
                sc.setSelectedSegmentIndex(idx);
            }
        }
    }

    // -----------------------------------------------------------------
    // Universal UIView attributes
    // -----------------------------------------------------------------

    /// Set this view's opacity (0.0..=1.0). Maps to UIView's `alpha`.
    pub fn set_alpha(&self, alpha: f64) {
        let v = self.ui_view();
        let clamped = alpha.clamp(0.0, 1.0);
        // UIView.alpha is CGFloat (f64 on 64-bit)
        if (v.alpha() - clamped).abs() > f64::EPSILON {
            v.setAlpha(clamped);
        }
    }

    // -----------------------------------------------------------------
    // Chrome — background, border, corner radius
    //
    // These all sit on UIView itself or its CALayer. We prefer the
    // UIKit setters where available (`setBackgroundColor` on UIView)
    // and fall through to the layer for the rest.
    // -----------------------------------------------------------------

    /// Set the view's background color. Pass `None` to clear (the
    /// view becomes transparent).
    pub fn set_background_color(&self, color: Option<crate::Color>) {
        let v = self.ui_view();
        match color {
            Some(c) => v.setBackgroundColor(Some(&c.to_uicolor())),
            None => v.setBackgroundColor(None),
        }
    }

    /// Set the view's corner radius (in points). Sets
    /// `layer.cornerRadius` and `layer.masksToBounds = true` so the
    /// rounded corners actually clip subview content. Pass `0.0` to
    /// disable rounding.
    pub fn set_corner_radius(&self, radius: f64) {
        let layer = self.ui_view().layer();
        if (layer.cornerRadius() - radius).abs() > f64::EPSILON {
            layer.setCornerRadius(radius);
            // Without masksToBounds the corners look correct from
            // afar but subview content (e.g. an image) bleeds past
            // the rounded edge. Always enable.
            layer.setMasksToBounds(radius > 0.0);
        }
    }

    /// Set the view's border width in points. `0.0` removes the
    /// border. Combine with [`set_border_color`](Self::set_border_color).
    pub fn set_border_width(&self, width: f64) {
        let layer = self.ui_view().layer();
        if (layer.borderWidth() - width).abs() > f64::EPSILON {
            layer.setBorderWidth(width);
        }
    }

    /// Set the view's border color. Pass `None` to clear.
    pub fn set_border_color(&self, color: Option<crate::Color>) {
        let layer = self.ui_view().layer();
        match color {
            Some(c) => {
                // UIColor::CGColor is unsafe in the binding (it
                // returns a Retained<CGColor> whose lifetime would
                // outlive the autorelease pool — fine here because
                // we hand it straight to setBorderColor which retains).
                let cg = unsafe { c.to_uicolor().CGColor() };
                layer.setBorderColor(Some(&cg));
            }
            None => layer.setBorderColor(None),
        }
    }

    // -----------------------------------------------------------------
    // Text styling
    // -----------------------------------------------------------------

    /// Set the text color on a text-bearing view (label, text_field,
    /// secure_text_field, text_view). UILabel doesn't inherit from
    /// UITextField, so we handle each type separately.
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

    /// Set text alignment.
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

    /// Set the font size (in points). Uses the system font at the
    /// given size; no Dynamic Type scaling in v1.
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
            crate::layout::schedule_relayout(&self.node);
        }
    }

    // -----------------------------------------------------------------
    // Control-specific statics
    // -----------------------------------------------------------------

    /// Toggle whether a UITextField draws a border.
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

    /// Toggle whether a UITextField draws its bezel (same as border on iOS).
    pub fn set_text_field_bezeled(&self, bezeled: bool) {
        self.set_text_field_bordered(bezeled);
    }

    /// Switch a UISlider between horizontal and vertical.
    /// Vertical sliders not natively supported by UISlider — we
    /// use a transform rotation. Stub for v1.
    pub fn set_slider_vertical(&self, _vertical: bool) {
        // Deferred: apply CGAffineTransform rotation
    }

    /// Set tick marks on a UISlider (not natively supported — no-op).
    pub fn set_slider_tick_marks(&self, _count: usize) {
        // UISlider doesn't have tick marks natively.
    }

    /// Snap-to-tick on a UISlider (no-op, no native support).
    pub fn set_slider_snaps_to_ticks(&self, _snaps: bool) {
    }

    /// UIDatePicker visual style.
    pub fn set_date_picker_style(&self, style: crate::DatePickerStyle) {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            dp.setPreferredDatePickerStyle(style.0);
        }
    }

    /// Constrain a UIDatePicker's selectable range.
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

    /// macOS-parity stub — iOS scroll indicators always auto-hide
    /// (they only appear during active scrolling and fade out
    /// shortly afterwards). Use `set_has_horizontal_scroller` /
    /// `set_has_vertical_scroller` for whether they appear at all.
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

    /// Toggle whether a UIProgressView stays visible when stopped.
    /// UIProgressView always stays visible — no-op.
    pub fn set_progress_displayed_when_stopped(&self, _shown: bool) {
    }

    /// Read the current date from a UIDatePicker.
    pub fn date_picker_value(&self) -> crate::Date {
        if let Some(dp) =
            downcast::<objc2_ui_kit::UIDatePicker>(self.ui_view())
        {
            let d = dp.date();
            return crate::Date::from_nsdate(&d);
        }
        crate::Date::now()
    }

    /// Set the date in a UIDatePicker.
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

    /// Read the value of a UIStepper.
    pub fn stepper_value(&self) -> f64 {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIStepper>(self.ui_view())
        {
            return s.value() as f64;
        }
        0.0
    }

    /// Set the value of a UIStepper.
    pub fn set_stepper_value(&self, v: f64) {
        if let Some(s) =
            downcast::<objc2_ui_kit::UIStepper>(self.ui_view())
        {
            if (s.value() as f64 - v).abs() > f64::EPSILON {
                s.setValue(v);
            }
        }
    }

    /// Configure a UIStepper's min, max, and step increment.
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

    /// Set the progress value on a UIProgressView (0..1).
    pub fn set_progress_value(&self, v: f64) {
        if let Some(p) =
            downcast::<objc2_ui_kit::UIProgressView>(self.ui_view())
        {
            p.setProgress(v as f32);
        }
    }

    /// Switch a UIProgressView between determinate (bar) and
    /// indeterminate. iOS doesn't have a native indeterminate
    /// progress bar — use UIActivityIndicatorView instead. Stub.
    pub fn set_progress_indeterminate(&self, _indeterminate: bool) {
    }

    /// Set the max value on a progress view. UIProgressView range
    /// is always 0..1 — no-op.
    pub fn set_progress_max(&self, _max: f64) {
    }

    /// Wire a change observer on a UITextView. Fires on every
    /// keystroke. Used by `bind:value` (write-back leg).
    pub fn on_text_view_change(
        &self,
        cb: impl FnMut(String) + 'static,
    ) {
        let view = self.ui_view();
        if let Some(tv) =
            downcast::<objc2_ui_kit::UITextView>(view)
        {
            crate::event::on_text_view_change(tv, cb);
        }
    }

    /// Set editability of a UITextView.
    pub fn set_text_view_editable(&self, editable: bool) {
        if let Some(tv) =
            downcast::<objc2_ui_kit::UITextView>(self.ui_view())
        {
            if tv.isEditable() != editable {
                tv.setEditable(editable);
            }
        }
    }

    /// Read the value of a UITextView. Returns None for non-
    /// text_view elements.
    pub fn text_view_value(&self) -> Option<String> {
        let tv =
            downcast::<objc2_ui_kit::UITextView>(self.ui_view())?;
        Some(tv.text().to_string())
    }

    /// Make this element the first responder (keyboard focus).
    pub fn focus(&self) -> bool {
        let view = self.ui_view();
        view.becomeFirstResponder()
    }

    /// Resign first responder.
    pub fn blur(&self) -> bool {
        let view = self.ui_view();
        view.resignFirstResponder()
    }

    /// Load an image into an `<image_view>` from a file path.
    /// Empty path clears the image.
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
        crate::layout::schedule_relayout(&self.node);
    }

    /// Load an image into an `<image_view>` from in-memory bytes.
    /// `None` or an empty slice clears the image; data UIKit can't
    /// decode also clears it. `UIImage::imageWithData:` auto-detects
    /// PNG, JPEG, GIF, TIFF, HEIC.
    ///
    /// Typical use: HTTP-fetch on a background async runtime, hand
    /// the bytes back to main thread via a channel, then this
    /// reactive setter fires on main.
    pub fn set_image_view_bytes(&self, bytes: Option<&[u8]>) {
        use objc2_ui_kit::{UIImage, UIImageView};
        use objc2_foundation::NSData;
        let Some(iv) = downcast::<UIImageView>(self.ui_view()) else {
            return;
        };
        let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
            iv.setImage(None);
            crate::layout::schedule_relayout(&self.node);
            return;
        };
        let data = NSData::with_bytes(bytes);
        let image = UIImage::imageWithData(&data);
        iv.setImage(image.as_deref());
        crate::layout::schedule_relayout(&self.node);
    }

    /// Resolve an SF Symbol name to a `UIImage`. Returns `None`
    /// for empty names or unknown symbols.
    fn sf_symbol_image(name: &str) -> Option<objc2::rc::Retained<objc2_ui_kit::UIImage>> {
        use objc2_ui_kit::UIImage;
        if name.is_empty() {
            return None;
        }
        let ns_name = NSString::from_str(name);
        UIImage::systemImageNamed(&ns_name)
    }

    /// Set an SF Symbol as the image on a `<button>` (UIButton)
    /// or `<image_view>` (UIImageView). Empty name clears.
    /// iOS 13+; no-op on older systems.
    pub fn set_sf_symbol(&self, name: &str) {
        let view = self.ui_view();
        let image = Self::sf_symbol_image(name);
        if let Some(button) = downcast::<UIButton>(view) {
            button.setImage_forState(
                image.as_deref(),
                objc2_ui_kit::UIControlState::Normal,
            );
            crate::layout::schedule_relayout(&self.node);
            return;
        }
        if let Some(iv) = downcast::<objc2_ui_kit::UIImageView>(view) {
            iv.setImage(image.as_deref());
            crate::layout::schedule_relayout(&self.node);
        }
    }

    /// Set the `tintColor` on the view. Applies to UIImageView,
    /// UIButton, and any UIView in general — UIKit propagates
    /// tint through SF Symbols (template images) automatically.
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

    // -----------------------------------------------------------------
    // Attribute removal
    // -----------------------------------------------------------------

    pub fn remove_attribute(&self, name: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.remove_string_attribute(attr);
            return;
        }
        if let Some(attr) = BoolAttr::from_name(name) {
            self.remove_bool_attribute(attr);
        }
    }

    pub fn remove_string_attribute(&self, attr: StringAttr) {
        let view = self.ui_view();
        match attr {
            StringAttr::Title => {
                if let Some(button) = downcast::<UIButton>(view) {
                    button.setTitle_forState(
                        Some(&NSString::from_str("")),
                        objc2_ui_kit::UIControlState::Normal,
                    );
                }
                if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
                    label.setText(Some(&NSString::from_str("")));
                }
            }
            StringAttr::Value => {
                if let Some(field) = downcast::<UITextField>(view) {
                    field.setText(Some(&NSString::from_str("")));
                } else if let Some(tv) = downcast::<objc2_ui_kit::UITextView>(view) {
                    tv.setText(Some(&NSString::from_str("")));
                }
            }
            StringAttr::Placeholder => {
                if let Some(field) = downcast::<UITextField>(view) {
                    field.setPlaceholder(None);
                }
            }
        }
    }

    pub fn remove_bool_attribute(&self, attr: BoolAttr) {
        match attr {
            BoolAttr::Hidden => self.set_bool_attribute(BoolAttr::Hidden, false),
            BoolAttr::Enabled => self.set_bool_attribute(BoolAttr::Enabled, true),
            BoolAttr::Checked => self.set_bool_attribute(BoolAttr::Checked, false),
        }
    }
}

// ---------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Text {
    node: Node,
}

impl Text {
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Text,
            "Text::from_node_unchecked called with a non-Text node"
        );
        Text { node }
    }

    pub fn create(content: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_with(content, mtm)
    }

    pub fn create_with(content: &str, mtm: MainThreadMarker) -> Self {
        use objc2_ui_kit::UILabel;
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let label = UILabel::initWithFrame(UILabel::alloc(mtm), frame);
        label.setText(Some(&NSString::from_str(content)));
        let view: Retained<UIView> = unsafe { Retained::cast_unchecked(label) };

        let mut style = crate::layout::Style::default();
        style.flex_shrink = 0.0;

        Text {
            node: Node::from_view(view, NodeKind::Text, style),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    pub fn set_text(&self, content: &str) {
        let view = self.node.ui_view();
        if let Some(label) = downcast::<objc2_ui_kit::UILabel>(view) {
            label.setText(Some(&NSString::from_str(content)));
        }
        crate::layout::schedule_relayout(&self.node);
    }
}

// ---------------------------------------------------------------------
// Placeholder
// ---------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Placeholder {
    node: Node,
}

impl Placeholder {
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Placeholder,
            "Placeholder::from_node_unchecked called with a \
             non-Placeholder node"
        );
        Placeholder { node }
    }

    pub fn create() -> Self {
        let mtm = MainThreadMarker::new()
            .expect("ios_dom must run on the main thread");
        Self::create_with(mtm)
    }

    pub fn create_with(mtm: MainThreadMarker) -> Self {
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

        Placeholder {
            node: Node::from_view(view, NodeKind::Placeholder, style),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Best-effort downcast of an `&UIView` to a more specific subclass.
fn downcast<T>(view: &UIView) -> Option<&T>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>()
}
