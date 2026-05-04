//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `Retained<NSView>`.
//!
//! Each Node carries a *shared* layout slot (Rc'd among clones)
//! holding its current Taffy [`Style`] plus an `Option<LayoutHandle>`.
//! The handle is `None` until the node is mounted into a tree (a
//! [`Window`](crate::app)'s `TaffyTree`); style mutations made before
//! that point are buffered locally and pushed to the tree at
//! registration time. See `crate::layout` for the registration helpers.
//!
//! See the crate-level docs for the threading contract.

use crate::layout::{LayoutHandle, NodeLayout, Style};
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSButton, NSControl, NSTextField, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use send_wrapper::SendWrapper;
use std::{cell::RefCell, fmt, rc::Rc};

/// Compile-time-checked attribute identifiers, split by value type.
///
/// Cocoa builders should use these typed enums when they know the
/// attribute name at the call site:
///   * String-valued attributes → [`StringAttr`] +
///     [`Element::set_string_attribute`]
///   * Bool-valued attributes → [`BoolAttr`] +
///     [`Element::set_bool_attribute`]
///
/// Passing the wrong-type variant to the wrong setter is a compile
/// error.
///
/// `Element::set_attribute(&str, &str)` and
/// `Element::remove_attribute(&str)` stay around for compatibility
/// with the `Rndr` trait (which is web-shaped and expects string
/// keys). Internally those route through `from_name` lookups on
/// the appropriate enum.
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

/// Distinguishes the three node varieties tachys cares about.
///
/// In the web DOM these correspond to Element / Text / Comment nodes.
/// We keep the distinction so `CastFrom<Node>` round-trips can validate
/// that a Node was originally created as an Element vs Text vs Placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Element,
    Text,
    Placeholder,
}

/// The core node wrapper.
///
/// `Node` is `Clone` (cheap retain + Rc bump) and `Send + 'static`
/// (via [`SendWrapper`]). It must only be touched on the main thread;
/// off-main access panics from the SendWrapper runtime check.
///
/// Cloning shares both the underlying NSView (via NSObject retain
/// counting on `Retained<NSView>`) and the layout slot (via `Rc`), so
/// every clone of a Node refers to the same NSView and the same
/// layout state. Style mutations made through one clone are visible
/// through every clone.
#[derive(Clone)]
pub struct Node {
    view: SendWrapper<Retained<NSView>>,
    layout: SendWrapper<Rc<RefCell<NodeLayout>>>,
    kind: NodeKind,
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr: *const NSView = &**self.view;
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
    /// Build a Node from an existing NSView (or subclass), with the
    /// given default style as its initial Taffy style. The Node starts
    /// *unregistered* — registration happens at mount time.
    pub fn from_view<V>(
        view: Retained<V>,
        kind: NodeKind,
        default_style: Style,
    ) -> Self
    where
        V: AsRef<NSView> + Message,
    {
        // Up-cast to the NSView base; the original subclass identity is
        // preserved through ObjC's dynamic dispatch.
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(view) };
        Node {
            view: SendWrapper::new(view),
            layout: SendWrapper::new(Rc::new(RefCell::new(NodeLayout::new(
                default_style,
            )))),
            kind,
        }
    }

    /// Build a Node wrapping `view` with a pre-existing
    /// [`LayoutHandle`] — i.e. one that already references a Taffy
    /// node in some tree. Used by `mount_before` in
    /// `tachys::renderer::cocoa::Dom` to synthesise a parent
    /// Element wrapper for an NSView whose Node we don't have, by
    /// borrowing the parent's LayoutHandle from a sibling node we do
    /// have.
    pub fn from_view_with_handle<V>(
        view: Retained<V>,
        kind: NodeKind,
        handle: LayoutHandle,
    ) -> Self
    where
        V: AsRef<NSView> + Message,
    {
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(view) };
        let layout = NodeLayout {
            style: Style::default(),
            handle: Some(handle),
        };
        Node {
            view: SendWrapper::new(view),
            layout: SendWrapper::new(Rc::new(RefCell::new(layout))),
            kind,
        }
    }

    /// Borrow the underlying NSView. Main-thread only.
    pub fn ns_view(&self) -> &NSView {
        &self.view
    }

    /// Take the wrapped Retained<NSView>. Main-thread only.
    pub fn into_ns_view(self) -> Retained<NSView> {
        self.view.take()
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// Borrow the (interior-mutable) layout slot. Used by the layout
    /// module to read/mutate the style and handle.
    pub fn layout_slot(&self) -> &RefCell<NodeLayout> {
        &**self.layout
    }

    /// Pointer-equality check (same underlying NSView object).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        let a: *const NSView = &**self.view;
        let b: *const NSView = &**other.view;
        a == b
    }

    /// Drop the resources owned by this node:
    ///   - any registered Taffy node (via [`crate::layout::drop_node`])
    ///   - any retained event-handler targets
    ///     (via [`crate::event::drop_handlers_for`])
    ///
    /// Then remove the underlying NSView from its superview.
    ///
    /// This is the cleanup hook called from `Mountable::unmount`
    /// implementations. Safe to call on a node with no registrations
    /// — the underlying drops are no-ops in that case.
    ///
    /// Note: this only tears down the *single* node, not its
    /// children. Recursive teardown happens via the tachys `Mountable`
    /// chain, where each parent's `unmount` recursively unmounts its
    /// children before tearing down itself.
    pub fn teardown(&self) {
        crate::event::drop_handlers_for(self.ns_view());
        crate::layout::drop_node(self);
        self.ns_view().removeFromSuperview();
    }
}

// ---------------------------------------------------------------------
// Element
// ---------------------------------------------------------------------

/// An element node — anything created by [`Element::create`] for a given
/// tag. Wraps a `Node` whose kind is `NodeKind::Element`.
#[derive(Clone, Debug)]
pub struct Element {
    node: Node,
}

impl Element {
    /// Wrap a `Node` whose kind has already been verified as
    /// `Element`. Panics in both debug and release if the kind is
    /// wrong — the check is a single enum compare, and the cost of
    /// silently allowing a Text/Placeholder to masquerade as an
    /// Element is much higher (silent no-ops in `set_attribute`).
    pub fn from_node_unchecked(node: Node) -> Self {
        assert_eq!(
            node.kind(),
            NodeKind::Element,
            "Element::from_node_unchecked called with a non-Element node"
        );
        Element { node }
    }

    /// Construct an element by tag name. Tag names map directly to
    /// AppKit view classes; see the crate root for the supported set.
    ///
    /// The element starts *unregistered* — its Taffy layout is set up
    /// only when it gets mounted under a parent that's already in a
    /// tree (i.e. eventually a [`Window`](crate::app) descendant).
    /// Style setters called before mount stash their values on the
    /// Node; on registration, the accumulated style becomes the
    /// initial Taffy style.
    ///
    /// # Panics
    /// Off the main thread.
    pub fn create(tag: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_with(tag, mtm)
    }

    pub fn create_with(tag: &str, mtm: MainThreadMarker) -> Self {
        use crate::{
            flipped_view::FlippedView,
            layout::{FlexDirection, Style},
        };

        // Default frame is a sentinel; layout overwrites it.
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));

        // For each tag, decide:
        //   1. Which AppKit class to instantiate.
        //   2. Whether the view is a layout container (needs flipped
        //      coords + a layout-friendly default Taffy style).
        let (view, default_style): (Retained<NSView>, Style) = match tag {
            "button" => {
                // Use `buttonWithTitle:target:action:` rather than
                // `initWithFrame:` — the former produces a properly-
                // styled push button (rounded bezel, ~32px tall,
                // sensible intrinsic size). Direct `initWithFrame`
                // gave us a button with a default bezel whose
                // intrinsic was a too-small 20px tall × text-only
                // wide, so titles like "Reset" rendered as "Rese".
                //
                // Title and target/action are set later via
                // `set_attribute("title", ...)` and `on_click(...)`.
                let b = unsafe {
                    NSButton::buttonWithTitle_target_action(
                        &NSString::from_str(""),
                        None,
                        None,
                        mtm,
                    )
                };
                let v: Retained<NSView> = unsafe { Retained::cast_unchecked(b) };
                (v, Style::default())
            }
            "checkbox" => {
                // `checkboxWithTitle:target:action:` produces an
                // NSButton pre-configured as a switch (checkbox bezel,
                // sensible intrinsic). Title and target/action are set
                // later via attributes / on_click.
                let b = unsafe {
                    NSButton::checkboxWithTitle_target_action(
                        &NSString::from_str(""),
                        None,
                        None,
                        mtm,
                    )
                };
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(b) };
                // Don't shrink — clipping a checkbox label looks bad.
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "label" => {
                let l = NSTextField::labelWithString(
                    &NSString::from_str(""),
                    mtm,
                );
                let v: Retained<NSView> = unsafe { Retained::cast_unchecked(l) };
                // No hardcoded size — measured via NSTextField's
                // intrinsic. Never shrink: NSTextField doesn't clip
                // its text content, so a frame shorter than the text
                // height results in text overflowing into siblings'
                // space.
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "text_field" => {
                let tf = NSTextField::initWithFrame(
                    NSTextField::alloc(mtm),
                    frame,
                );
                let v: Retained<NSView> = unsafe { Retained::cast_unchecked(tf) };
                // Same as `label`: measured via NSTextField intrinsic,
                // never shrink (editable content shouldn't get clipped
                // by sibling overlap).
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "slider" => {
                use objc2_app_kit::NSSlider;
                let s = NSSlider::initWithFrame(NSSlider::alloc(mtm), frame);
                // Continuous: fire target/action on every drag-update,
                // not just on mouse-up. Web-equivalent expectation
                // (input event firing while sliding).
                s.setContinuous(true);
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(s) };
                // Sliders have a defined intrinsic height; let the
                // parent decide width via the cross-axis stretch.
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "pop_up_button" => {
                use objc2_app_kit::NSPopUpButton;
                let p = NSPopUpButton::initWithFrame_pullsDown(
                    NSPopUpButton::alloc(mtm),
                    frame,
                    false,
                );
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(p) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "secure_text_field" => {
                use objc2_app_kit::NSSecureTextField;
                let tf = NSSecureTextField::initWithFrame(
                    NSSecureTextField::alloc(mtm),
                    frame,
                );
                // Cast straight to NSView; downstream code that
                // downcasts to NSTextField still works because
                // NSSecureTextField IS-A NSTextField.
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(tf) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "stack_view" => {
                // Stage-4 choice: don't use NSStackView's own
                // constraint-based layout — Taffy is the single source
                // of truth. A flipped NSView with a column default does
                // the same job and stays consistent.
                let v: Retained<NSView> = unsafe {
                    Retained::cast_unchecked(FlippedView::new(mtm))
                };
                let mut s = Style::default();
                s.flex_direction = FlexDirection::Column;
                (v, s)
            }
            // "view" or anything unknown → generic flipped container
            _ => {
                let v: Retained<NSView> = unsafe {
                    Retained::cast_unchecked(FlippedView::new(mtm))
                };
                (v, Style::default())
            }
        };

        Element {
            node: Node::from_view(view, NodeKind::Element, default_style),
        }
    }

    pub fn as_node(&self) -> &Node {
        &self.node
    }

    pub fn into_node(self) -> Node {
        self.node
    }

    pub fn ns_view(&self) -> &NSView {
        self.node.ns_view()
    }

    /// Insert `child` before `marker` in this element's child list.
    /// If `marker` is `None`, append.
    ///
    /// Mirrors `Node.insertBefore` from the web DOM.
    ///
    /// If this element is registered in a Taffy tree, the child is
    /// registered in the same tree (cascading to the child's own
    /// descendants if the insert is the first time the subtree gets
    /// connected to a tree-rooted ancestor). Otherwise it's a pure
    /// NSView-level mutation.
    pub fn insert_node(&self, child: &Node, marker: Option<&Node>) {
        let parent = self.ns_view();
        let child_view = child.ns_view();

        match marker {
            None => {
                parent.addSubview(child_view);
                crate::layout::attach_child(self.as_node(), child);
            }
            Some(marker) => {
                let marker_view = marker.ns_view();
                splice_subview_before(parent, child_view, marker_view);
                // Find where `child` ended up in the subview array,
                // mirror the same index into Taffy.
                let subviews = parent.subviews();
                let child_ptr: *const NSView = child_view;
                let mut child_index = subviews.len();
                for (i, sv) in subviews.iter().enumerate() {
                    let sv_ptr: *const NSView = &*sv;
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

    /// Remove `child` from this element. Returns the node back if it was
    /// actually our child, otherwise `None`.
    pub fn remove_child(&self, child: &Node) -> Option<Node> {
        let parent_ptr: *const NSView = self.ns_view();
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
        crate::layout::detach_child(self.as_node(), child);
        Some(child.clone())
    }

    /// Remove every child.
    ///
    /// Note: detaches from NSView only. The children's Taffy entries
    /// stay registered (they'll be cleaned up via the
    /// `Mountable::unmount` chain when their owners drop). We don't
    /// have a registry to walk back from raw subview to its Node, so
    /// can't `detach_child` for each one here.
    pub fn clear_children(&self) {
        let parent = self.ns_view();
        // subviews returns a copy, so iterating + removing is safe.
        let subs = parent.subviews();
        for sv in subs.iter() {
            sv.removeFromSuperview();
        }
    }

    /// `&str`-keyed entry point matching the `Rndr` trait. Routes
    /// through the typed enums — silently no-ops on unknown names.
    /// Internal cocoa builders should prefer the typed
    /// [`set_string_attribute`](Self::set_string_attribute) and
    /// [`set_bool_attribute`](Self::set_bool_attribute) directly.
    pub fn set_attribute(&self, name: &str, value: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.set_string_attribute(attr, value);
        }
        // Bool attrs through this entry point would require parsing
        // "true"/"false" — we deliberately don't, since the typed
        // setter is the only blessed path. Unknown names no-op.
    }

    /// Typed string-valued attribute setter. Routing:
    ///   * `Title`       → `NSButton::setTitle:`
    ///   * `Value`       → `NSControl::setStringValue:`
    ///   * `Placeholder` → `NSTextField::setPlaceholderString:`
    pub fn set_string_attribute(&self, attr: StringAttr, value: &str) {
        let view = self.ns_view();
        let mut content_changed = false;
        match attr {
            StringAttr::Title => {
                if let Some(button) = downcast::<NSButton>(view) {
                    // Skip if the title hasn't actually changed —
                    // avoids a needless layout/redraw cycle when the
                    // same value is re-applied (e.g. by a bind:
                    // Effect after user typing already left the title
                    // unchanged).
                    let current = button.title().to_string();
                    if current != value {
                        button.setTitle(&NSString::from_str(value));
                        content_changed = true;
                    }
                }
            }
            StringAttr::Value => {
                if let Some(control) = downcast::<NSControl>(view) {
                    let current = control.stringValue().to_string();
                    if current != value {
                        control.setStringValue(&NSString::from_str(value));
                        content_changed = true;
                    }
                }
            }
            StringAttr::Placeholder => {
                if let Some(field) = downcast::<NSTextField>(view) {
                    // Diff before mutating: NSTextField doesn't redraw
                    // on same-value sets but we keep the parity with
                    // Title/Value behavior anyway.
                    let current: String = field
                        .placeholderString()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    if current != value {
                        let s = NSString::from_str(value);
                        field.setPlaceholderString(Some(&s));
                        // Placeholder text shows when the field is
                        // empty; its width contributes to NSTextField's
                        // intrinsicContentSize. Mark dirty so Taffy
                        // re-measures.
                        content_changed = true;
                    }
                }
            }
        }
        if content_changed {
            // Intrinsic size may have changed; trigger a relayout
            // pass so the frame catches up to the new content.
            crate::layout::schedule_relayout(&self.node);
        }
    }

    /// Typed boolean-valued attribute setter. Routing:
    ///   * `Enabled` → `NSControl::setEnabled:`
    ///   * `Hidden`  → `NSView::setHidden:`
    ///   * `Checked` → `NSButton::setState:` (On / Off)
    ///
    /// Each setter diffs against the current AppKit value before
    /// mutating (avoids redundant redraws / focus-ring flashes).
    pub fn set_bool_attribute(&self, attr: BoolAttr, value: bool) {
        let view = self.ns_view();
        match attr {
            BoolAttr::Hidden => {
                if view.isHidden() != value {
                    view.setHidden(value);
                }
            }
            BoolAttr::Enabled => {
                if let Some(control) = downcast::<NSControl>(view) {
                    if control.isEnabled() != value {
                        control.setEnabled(value);
                    }
                }
            }
            BoolAttr::Checked => {
                if let Some(button) = downcast::<NSButton>(view) {
                    use objc2_app_kit::{
                        NSControlStateValueOff, NSControlStateValueOn,
                    };
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
        }
    }

    /// Wire a click handler to this element. No-op if this element
    /// isn't an NSButton-class instance — silently dropped, matching
    /// the web `addEventListener` shape.
    ///
    /// Limitation: each call replaces NSControl's stored target/action
    /// pair, so calling twice keeps the latest handler only.
    /// (Multiple-listener support will need a fan-out target that
    /// holds a Vec<Box<dyn FnMut>>.)
    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        if let Some(button) = downcast::<NSButton>(self.ns_view()) {
            crate::event::on_control_action(button.as_ref(), cb);
        }
    }

    /// Wire a callback that fires when an NSControl's value changes
    /// (slider drag, popup selection, button click — any
    /// target/action). No-op if this element isn't an NSControl.
    ///
    /// This is the generic version of [`on_click`]; use it for
    /// slider/popup-style controls where "click" is misleading.
    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        if let Some(c) = downcast::<NSControl>(self.ns_view()) {
            crate::event::on_control_action(c, cb);
        }
    }

    /// Wire a callback that fires whenever the text content of a
    /// text-field changes (every keystroke / paste / etc.). No-op
    /// if this element isn't an NSTextField. Multiple handlers are
    /// supported — each call appends to the field's fan-out
    /// delegate.
    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_change(field, cb);
        }
    }

    /// Wire a callback that fires when the user commits an edit
    /// (return key, focus loss, tabbing away). No-op if this
    /// element isn't an NSTextField. Coexists with
    /// `on_text_change` (both can be installed on one field).
    /// Read the on/off state of an NSButton (checkbox / switch /
    /// other toggle types). Returns `false` for non-button views.
    pub fn checked(&self) -> bool {
        if let Some(button) = downcast::<NSButton>(self.ns_view()) {
            use objc2_app_kit::NSControlStateValueOn;
            return button.state() == NSControlStateValueOn;
        }
        false
    }

    /// Read the current `doubleValue` of an NSControl. Useful for
    /// sliders. Returns 0.0 for non-NSControl views.
    pub fn double_value(&self) -> f64 {
        if let Some(c) = downcast::<NSControl>(self.ns_view()) {
            return c.doubleValue();
        }
        0.0
    }

    /// Set the `doubleValue` on an NSControl (slider, etc.). Diffs to
    /// avoid redundant redraws; also no-ops on non-NSControl views.
    pub fn set_double_value(&self, v: f64) {
        if let Some(c) = downcast::<NSControl>(self.ns_view()) {
            if (c.doubleValue() - v).abs() > f64::EPSILON {
                c.setDoubleValue(v);
            }
        }
    }

    /// Slider min. Calls `setMinValue:` on NSSlider; no-op on
    /// non-slider views.
    pub fn set_slider_min(&self, v: f64) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = downcast::<NSSlider>(self.ns_view()) {
            s.setMinValue(v);
        }
    }

    /// Slider max.
    pub fn set_slider_max(&self, v: f64) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = downcast::<NSSlider>(self.ns_view()) {
            s.setMaxValue(v);
        }
    }

    /// Replace the items list on an NSPopUpButton. Selected index is
    /// reset to 0 (AppKit default behavior). No-op on non-popup views.
    pub fn set_popup_items(&self, items: &[String]) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = downcast::<NSPopUpButton>(self.ns_view()) {
            p.removeAllItems();
            for it in items {
                p.addItemWithTitle(&NSString::from_str(it));
            }
        }
    }

    /// Currently-selected index on an NSPopUpButton (-1 if nothing
    /// selected). Returns -1 for non-popup views as well.
    pub fn popup_selection(&self) -> isize {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = downcast::<NSPopUpButton>(self.ns_view()) {
            return p.indexOfSelectedItem();
        }
        -1
    }

    /// Programmatically pick an item by index. Diffs first to avoid
    /// the redundant-write cycle that bind: would otherwise flash.
    pub fn set_popup_selection(&self, idx: isize) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = downcast::<NSPopUpButton>(self.ns_view()) {
            if p.indexOfSelectedItem() != idx {
                p.selectItemAtIndex(idx);
            }
        }
    }

    pub fn on_text_end_editing(&self, cb: impl FnMut(String) + 'static) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_end_editing(field, cb);
        }
    }

    /// Wire a callback that fires when the text field gains focus
    /// (`controlTextDidBeginEditing:`). No-op on non-NSTextField.
    pub fn on_text_focus(&self, cb: impl FnMut() + 'static) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_focus(field, cb);
        }
    }

    /// Wire a callback that fires when the text field loses focus
    /// (Return / Tab / click-elsewhere — same notification as
    /// `on_text_end_editing` but with no value payload). No-op
    /// on non-NSTextField.
    pub fn on_text_blur(&self, cb: impl FnMut() + 'static) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_blur(field, cb);
        }
    }

    /// `&str`-keyed entry point matching the `Rndr` trait. Looks
    /// up the name in both [`StringAttr`] and [`BoolAttr`]; silently
    /// no-ops on unknown names.
    pub fn remove_attribute(&self, name: &str) {
        if let Some(attr) = StringAttr::from_name(name) {
            self.remove_string_attribute(attr);
            return;
        }
        if let Some(attr) = BoolAttr::from_name(name) {
            self.remove_bool_attribute(attr);
        }
    }

    /// Reset a string attribute to empty/None.
    pub fn remove_string_attribute(&self, attr: StringAttr) {
        let view = self.ns_view();
        match attr {
            StringAttr::Title => {
                if let Some(button) = downcast::<NSButton>(view) {
                    button.setTitle(&NSString::from_str(""));
                }
            }
            StringAttr::Value => {
                if let Some(control) = downcast::<NSControl>(view) {
                    control.setStringValue(&NSString::from_str(""));
                }
            }
            StringAttr::Placeholder => {
                if let Some(field) = downcast::<NSTextField>(view) {
                    field.setPlaceholderString(None);
                }
            }
        }
    }

    /// Reset a bool attribute to its default-absent value:
    ///   * `Hidden`  → `false` (visible)
    ///   * `Enabled` → `true` (enabled — NSControl's default)
    ///   * `Checked` → `false` (off)
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

/// A text node. Backed by a non-editable, non-bordered NSTextField
/// (AppKit's standard "label" configuration).
#[derive(Clone, Debug)]
pub struct Text {
    node: Node,
}

impl Text {
    /// Wrap a `Node` whose kind has already been verified as `Text`.
    /// Panics in debug *and* release if the kind is wrong — see
    /// [`Element::from_node_unchecked`] for rationale.
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
            .expect("cocoa_dom must run on the main thread");
        Self::create_with(content, mtm)
    }

    pub fn create_with(content: &str, mtm: MainThreadMarker) -> Self {
        let label = NSTextField::labelWithString(
            &NSString::from_str(content),
            mtm,
        );
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(label) };

        // Size is measured from `intrinsicContentSize` at layout time
        // (font metrics + actual string content). Never shrink:
        // NSTextField doesn't clip its text, so a too-small frame
        // overflows into sibling space.
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

    /// Update the displayed string.
    pub fn set_text(&self, content: &str) {
        let view = self.node.ns_view();
        // We created this as an NSTextField; downcast and set its value.
        if let Some(field) = downcast::<NSTextField>(view) {
            field.setStringValue(&NSString::from_str(content));
        }
        // Content changed → intrinsic size may have changed too.
        // Schedule a relayout pass so the label's frame catches up.
        crate::layout::schedule_relayout(&self.node);
    }
}

// ---------------------------------------------------------------------
// Placeholder
// ---------------------------------------------------------------------

/// A placeholder node — has a position in the tree but no visible
/// representation. Used by tachys to anchor dynamic content (the moral
/// equivalent of an HTML comment node used as a marker).
///
/// Backed by an empty hidden NSView. Default Taffy style is
/// `position: absolute; size: 0×0` so it doesn't take a slot in the
/// parent's flex layout (tachys' `Render for ()` builds Placeholders;
/// without the absolute positioning, every empty `()` would offset
/// its siblings by `gap`).
#[derive(Clone, Debug)]
pub struct Placeholder {
    node: Node,
}

impl Placeholder {
    /// Wrap a `Node` whose kind has already been verified as
    /// `Placeholder`. Panics in debug *and* release if the kind is
    /// wrong — see [`Element::from_node_unchecked`] for rationale.
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
            .expect("cocoa_dom must run on the main thread");
        Self::create_with(mtm)
    }

    pub fn create_with(mtm: MainThreadMarker) -> Self {
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
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

/// Best-effort downcast of an `&NSView` to a more specific subclass.
/// Returns `None` if the runtime class isn't a subclass of `T`.
fn downcast<T>(view: &NSView) -> Option<&T>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>()
}

/// Insert `child` immediately before `marker` in `parent`'s subview
/// array. O(1): we use AppKit's `addSubview:positioned:relativeTo:`
/// with `NSWindowOrderingMode::Below`, which inserts the new subview
/// right before `marker` in the subviews ordering.
fn splice_subview_before(parent: &NSView, child: &NSView, marker: &NSView) {
    parent.addSubview_positioned_relativeTo(
        child,
        NSWindowOrderingMode::Below,
        Some(marker),
    );
}
