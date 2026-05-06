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

use crate::layout::{Dimension, LayoutHandle, NodeLayout, Style};
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
            is_scroll_view: false,
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
        // For NSScrollView-backed elements (`<text_view>` and
        // `<scroll_view>`), the documentView holds its own handler
        // store entries (e.g. NSTextView's bind:value delegate).
        // Walk one level deeper so they don't leak.
        let view = self.ns_view();
        if let Some(scroll) = {
            use objc2_app_kit::NSScrollView;
            let any: &objc2::runtime::AnyObject = view.as_ref();
            any.downcast_ref::<NSScrollView>()
        } {
            if let Some(doc) = scroll.documentView() {
                crate::event::drop_handlers_for(&doc);
            }
        }
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
            "date_picker" => {
                use objc2_app_kit::NSDatePicker;
                let dp = NSDatePicker::initWithFrame(
                    NSDatePicker::alloc(mtm),
                    frame,
                );
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(dp) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "stepper" => {
                use objc2_app_kit::NSStepper;
                let st = NSStepper::initWithFrame(
                    NSStepper::alloc(mtm),
                    frame,
                );
                // Wrap negative steps around max (web-shaped); user
                // can disable via setValueWraps if they want clamp
                // behavior.
                st.setValueWraps(false);
                // Continuous = fire on every drag tick, not just on
                // mouse-up. Matches slider's default; consistent
                // expectation for live-update controls.
                st.setAutorepeat(true);
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(st) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "progress_indicator" => {
                use objc2_app_kit::NSProgressIndicator;
                let pi = NSProgressIndicator::initWithFrame(
                    NSProgressIndicator::alloc(mtm),
                    frame,
                );
                // Default: bar style, determinate, 0..1 range.
                // `indeterminate=true` switches to spinner; user-
                // controllable via the builder's `.indeterminate(b)`.
                pi.setMinValue(0.0);
                pi.setMaxValue(1.0);
                pi.setIndeterminate(false);
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(pi) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "color_well" => {
                use objc2_app_kit::NSColorWell;
                let cw = NSColorWell::initWithFrame(
                    NSColorWell::alloc(mtm),
                    frame,
                );
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(cw) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "segmented_control" => {
                use objc2_app_kit::NSSegmentedControl;
                let sc = NSSegmentedControl::initWithFrame(
                    NSSegmentedControl::alloc(mtm),
                    frame,
                );
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(sc) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "scroll_view" => {
                // NSScrollView wrapping a FlippedView documentView.
                // Children added via `insert_node` are routed to the
                // documentView via `Element::subview_parent`. The
                // scroll view's *outer* frame is whatever the parent
                // gives it (the viewport); the documentView is sized
                // separately in a second `compute_layout` pass with
                // `MaxContent` height — see
                // `compute_layout_scroll_views` in layout.rs.
                use objc2_app_kit::NSScrollView;
                let scroll = NSScrollView::initWithFrame(
                    NSScrollView::alloc(mtm),
                    frame,
                );
                scroll.setHasVerticalScroller(true);
                scroll.setHasHorizontalScroller(false);
                scroll.setBorderType(
                    objc2_app_kit::NSBorderType::NoBorder,
                );

                let doc: Retained<NSView> = unsafe {
                    Retained::cast_unchecked(FlippedView::new(mtm))
                };
                scroll.setDocumentView(Some(&doc));

                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(scroll) };
                // CSS pattern for "shrinkable scroll container":
                //   flex-basis: 0; min-height: 0; overflow: hidden.
                // Without flex_basis=0, the flex algorithm would
                // start scroll_view at its content size (e.g. 538
                // tall for a list of 30 rows), and flex-grow can't
                // bring it back down — outer ancestors would then
                // grow past the window. With flex_basis=0 and
                // min_size=0, scroll_view collapses to nothing by
                // default and only the user's flex_grow / explicit
                // height grows it back to the viewport.
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
            "image_view" => {
                use objc2_app_kit::{NSImageView, NSImageScaling};
                let iv = NSImageView::initWithFrame(
                    NSImageView::alloc(mtm),
                    frame,
                );
                // Default to scaling-down-only with aspect ratio
                // preserved — images larger than the frame fit
                // inside; smaller images render at native size.
                iv.setImageScaling(
                    NSImageScaling::ScaleProportionallyDown,
                );
                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(iv) };
                let mut s = Style::default();
                s.flex_shrink = 0.0;
                (v, s)
            }
            "text_view" => {
                // NSTextView is multi-line, rich-text-capable text
                // editing. Standard AppKit pattern is to embed it
                // inside an NSScrollView so overflow scrolls
                // (NSTextView itself doesn't scroll). We wrap the
                // scroll view and treat the contained text view as
                // an internal implementation detail; setters route
                // through `documentView()` (see `set_string_attribute`
                // for `StringAttr::Value`).
                //
                // Limitations of this v1:
                //   * No event hooks (NSTextViewDelegate is a
                //     separate protocol from
                //     NSControlTextEditingDelegate). Add when
                //     needed.
                //   * Plain text only (`setRichText(false)`).
                use objc2_app_kit::{NSScrollView, NSTextView};
                let scroll = NSScrollView::initWithFrame(
                    NSScrollView::alloc(mtm),
                    frame,
                );
                scroll.setHasVerticalScroller(true);
                scroll.setHasHorizontalScroller(false);
                scroll.setBorderType(objc2_app_kit::NSBorderType::BezelBorder);

                let tv = NSTextView::initWithFrame(
                    NSTextView::alloc(mtm),
                    frame,
                );
                tv.setEditable(true);
                tv.setSelectable(true);
                tv.setRichText(false);
                tv.setImportsGraphics(false);
                scroll.setDocumentView(Some(&tv));

                let v: Retained<NSView> =
                    unsafe { Retained::cast_unchecked(scroll) };
                // Don't shrink past content; multi-line editing
                // surfaces shouldn't get squeezed by sibling
                // overlap.
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
            "stack" => {
                // Canonical linear-layout primitive — flexbox with no
                // axis preset (the builder layer chooses Row/Column).
                let v: Retained<NSView> = unsafe {
                    Retained::cast_unchecked(FlippedView::new(mtm))
                };
                (v, Style::default())
            }
            #[cfg(feature = "block_layout")]
            "block" => {
                // Block-layout container — children stack vertically and
                // fill container width by default.
                let v: Retained<NSView> = unsafe {
                    Retained::cast_unchecked(FlippedView::new(mtm))
                };
                let mut s = Style::default();
                s.display = crate::layout::Display::Block;
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

        let node = Node::from_view(view, NodeKind::Element, default_style);
        // Flag scroll_view so the layout engine knows to do a
        // second compute_layout pass on its subtree (see
        // `compute_layout_scroll_views` in layout.rs).
        if tag == "scroll_view" {
            node.layout_slot().borrow_mut().is_scroll_view = true;
        }

        Element { node }
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

    /// The NSView that *actually* parents this element's children.
    /// For most tags this is just `self.ns_view()`. For
    /// `<scroll_view>` it's the NSScrollView's documentView — a
    /// FlippedView we install at construction. Routing through this
    /// helper lets `<scroll_view>` participate in the normal
    /// insert/remove/layout machinery without each call site
    /// special-casing it.
    ///
    /// Gated on the `is_scroll_view` Node flag rather than a
    /// dynamic NSScrollView class check, so `<text_view>` (also
    /// backed by an NSScrollView, but with an opaque NSTextView
    /// documentView) is not affected — its children would otherwise
    /// be misrouted into the NSTextView.
    pub fn subview_parent(&self) -> Retained<NSView> {
        let direct = self.ns_view();
        let routes_to_doc =
            self.node.layout_slot().borrow().is_scroll_view;
        if routes_to_doc {
            if let Some(scroll) =
                downcast::<objc2_app_kit::NSScrollView>(direct)
            {
                if let Some(doc) = scroll.documentView() {
                    return doc;
                }
            }
        }
        direct.into()
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
        let parent_retained = self.subview_parent();
        let parent: &NSView = &parent_retained;
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
        let parent_retained = self.subview_parent();
        let parent: &NSView = &parent_retained;
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
                } else if let Some(scroll) =
                    downcast::<objc2_app_kit::NSScrollView>(view)
                {
                    // text_view is an NSScrollView wrapping an
                    // NSTextView (the document view). Route value
                    // mutations through to the inner NSTextView.
                    if let Some(doc) = scroll.documentView() {
                        let any_doc: &objc2::runtime::AnyObject = &doc;
                        if let Some(tv) = any_doc
                            .downcast_ref::<objc2_app_kit::NSTextView>()
                        {
                            let current = tv.string().to_string();
                            if current != value {
                                tv.setString(&NSString::from_str(value));
                                content_changed = true;
                            }
                        }
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

    /// Replace the labels on an NSSegmentedControl. Resizes the
    /// control's segment count to match. Selected segment is reset
    /// to -1 (none selected) if the count shrinks below the
    /// previous selection. No-op on non-segmented views.
    pub fn set_segmented_items(&self, items: &[String]) {
        use objc2_app_kit::NSSegmentedControl;
        let Some(sc) =
            downcast::<NSSegmentedControl>(self.ns_view())
        else {
            return;
        };
        sc.setSegmentCount(items.len() as isize);
        for (i, label) in items.iter().enumerate() {
            sc.setLabel_forSegment(
                &NSString::from_str(label),
                i as isize,
            );
        }
    }

    /// Currently-selected segment on an NSSegmentedControl (-1 if
    /// nothing selected). Returns -1 for non-segmented views.
    pub fn segmented_selection(&self) -> isize {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) =
            downcast::<NSSegmentedControl>(self.ns_view())
        {
            return sc.selectedSegment();
        }
        -1
    }

    /// Programmatically pick a segment by index. Diffs first.
    pub fn set_segmented_selection(&self, idx: isize) {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) =
            downcast::<NSSegmentedControl>(self.ns_view())
        {
            if sc.selectedSegment() != idx {
                sc.setSelectedSegment(idx);
            }
        }
    }

    // -----------------------------------------------------------------
    // Universal NSView attributes
    // -----------------------------------------------------------------

    /// Set this view's opacity (0.0..=1.0). Maps to NSView's
    /// `alphaValue`. Diff-guarded.
    pub fn set_alpha(&self, alpha: f64) {
        let v = self.ns_view();
        let clamped = alpha.clamp(0.0, 1.0);
        if (v.alphaValue() - clamped).abs() > f64::EPSILON {
            v.setAlphaValue(clamped);
        }
    }

    /// Set this view's tool tip (the text shown when the mouse
    /// hovers over the view, after a brief delay). Empty string
    /// removes the tooltip.
    pub fn set_tool_tip(&self, tip: &str) {
        let v = self.ns_view();
        if tip.is_empty() {
            v.setToolTip(None);
        } else {
            let s = NSString::from_str(tip);
            v.setToolTip(Some(&s));
        }
    }

    // -----------------------------------------------------------------
    // Text styling (NSTextField, NSButton, NSTextView)
    // -----------------------------------------------------------------

    /// Set the text color on a text-bearing view (label,
    /// text_field, secure_text_field, or text_view). NSButton's
    /// text color isn't trivially settable (needs an
    /// `attributedTitle` round-trip with NSAttributedString); we
    /// don't expose it here. No-op on other kinds.
    pub fn set_text_color(&self, color: crate::Color) {
        let view = self.ns_view();
        let nscolor = color.to_nscolor();

        if let Some(field) = downcast::<NSTextField>(view) {
            field.setTextColor(Some(&nscolor));
            return;
        }
        if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
        {
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

    /// Set text alignment on a text-bearing view. No-op on others.
    pub fn set_text_alignment(
        &self,
        alignment: objc2_app_kit::NSTextAlignment,
    ) {
        let view = self.ns_view();

        if let Some(field) = downcast::<NSTextField>(view) {
            field.setAlignment(alignment);
            return;
        }
        if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
        {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    tv.setAlignment(alignment);
                }
            }
        }
    }

    /// Set the font size (in points) on a text-bearing view. Uses
    /// the system font at the given size; size mapping is verbatim
    /// (no Dynamic Type scaling). No-op on others.
    pub fn set_font_size(&self, points: f64) {
        use objc2_app_kit::NSFont;
        let font = NSFont::systemFontOfSize(points);

        let view = self.ns_view();
        if let Some(field) = downcast::<NSTextField>(view) {
            field.setFont(Some(&font));
            return;
        }
        if let Some(button) = downcast::<NSButton>(view) {
            button.setFont(Some(&font));
            return;
        }
        if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
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
        crate::layout::schedule_relayout(&self.node);
    }

    // -----------------------------------------------------------------
    // Control-specific statics
    // -----------------------------------------------------------------

    /// Toggle whether an NSButton draws its bezel (the rounded
    /// pill background). Borderless buttons sit flat against
    /// their container — useful for toolbar / link-style
    /// affordances. No-op on non-buttons.
    pub fn set_button_bordered(&self, bordered: bool) {
        if let Some(b) = downcast::<NSButton>(self.ns_view()) {
            b.setBordered(bordered);
        }
    }

    /// Set the keyboard shortcut for an NSButton. Pass `"\r"`
    /// (Return) to make this the default action button, `"\u{1b}"`
    /// (Escape) for the cancel button, or any single-character
    /// string. Empty string clears the shortcut. No-op on
    /// non-buttons.
    pub fn set_key_equivalent(&self, key: &str) {
        if let Some(b) = downcast::<NSButton>(self.ns_view()) {
            b.setKeyEquivalent(&NSString::from_str(key));
        }
    }

    /// Toggle whether an NSTextField draws a border / bezel.
    /// `bordered=false` matches a label-style appearance even on
    /// editable fields. No-op on non-NSTextField.
    pub fn set_text_field_bordered(&self, bordered: bool) {
        if let Some(f) = downcast::<NSTextField>(self.ns_view()) {
            f.setBordered(bordered);
        }
    }

    /// Toggle whether an NSTextField draws its bezel (the inset
    /// 3D look). Off → flat. No-op on non-NSTextField.
    pub fn set_text_field_bezeled(&self, bezeled: bool) {
        if let Some(f) = downcast::<NSTextField>(self.ns_view()) {
            f.setBezeled(bezeled);
        }
    }

    /// Toggle whether a label can be selected (text-copyable).
    /// No-op on non-NSTextField.
    pub fn set_selectable(&self, selectable: bool) {
        if let Some(f) = downcast::<NSTextField>(self.ns_view()) {
            f.setSelectable(selectable);
        }
    }

    /// Switch an NSSlider between horizontal and vertical
    /// orientation. AppKit auto-rotates the track based on the
    /// slider's frame ratio by default; calling this forces a
    /// specific orientation. No-op on non-sliders.
    pub fn set_slider_vertical(&self, vertical: bool) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = downcast::<NSSlider>(self.ns_view()) {
            s.setVertical(vertical);
        }
    }

    /// Set the number of evenly-spaced tick marks on an NSSlider.
    /// 0 hides ticks entirely. No-op on non-sliders.
    pub fn set_slider_tick_marks(&self, count: usize) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = downcast::<NSSlider>(self.ns_view()) {
            s.setNumberOfTickMarks(count as isize);
        }
    }

    /// Toggle "snap to tick" behavior on an NSSlider. When `true`,
    /// dragging snaps to the nearest tick mark.
    pub fn set_slider_snaps_to_ticks(&self, snaps: bool) {
        use objc2_app_kit::NSSlider;
        if let Some(s) = downcast::<NSSlider>(self.ns_view()) {
            s.setAllowsTickMarkValuesOnly(snaps);
        }
    }

    /// Switch an NSPopUpButton between popup mode (`pulls_down=
    /// false`, default) and pull-down mode (`pulls_down=true`,
    /// where the button keeps its fixed title and the menu items
    /// are actions rather than selections). No-op on non-popups.
    pub fn set_pulls_down(&self, pulls_down: bool) {
        use objc2_app_kit::NSPopUpButton;
        if let Some(p) = downcast::<NSPopUpButton>(self.ns_view()) {
            p.setPullsDown(pulls_down);
        }
    }

    /// Set an NSSegmentedControl's visual style. See
    /// `NSSegmentStyle` for the options
    /// (Rounded, RoundRect, Capsule, etc.). No-op on
    /// non-segmented.
    pub fn set_segment_style(
        &self,
        style: objc2_app_kit::NSSegmentStyle,
    ) {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) =
            downcast::<NSSegmentedControl>(self.ns_view())
        {
            sc.setSegmentStyle(style);
        }
    }

    /// Set NSDatePicker's visual style (textual / textual+stepper /
    /// clock-and-calendar). No-op on non-date-pickers.
    pub fn set_date_picker_style(
        &self,
        style: objc2_app_kit::NSDatePickerStyle,
    ) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = downcast::<NSDatePicker>(self.ns_view()) {
            dp.setDatePickerStyle(style);
        }
    }

    /// Constrain an NSDatePicker's selectable range. Pass `None`
    /// to unset (allow all dates).
    pub fn set_date_picker_min(&self, d: Option<crate::Date>) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = downcast::<NSDatePicker>(self.ns_view()) {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMinDate(nd.as_deref());
        }
    }

    pub fn set_date_picker_max(&self, d: Option<crate::Date>) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = downcast::<NSDatePicker>(self.ns_view()) {
            let nd = d.map(|d| d.to_nsdate());
            dp.setMaxDate(nd.as_deref());
        }
    }

    /// Toggle auto-hiding of an NSScrollView's scrollers (they
    /// fade out when not in use). No-op on non-scroll-views.
    pub fn set_autohides_scrollers(&self, autohides: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = downcast::<NSScrollView>(self.ns_view()) {
            s.setAutohidesScrollers(autohides);
        }
    }

    /// Show/hide an NSScrollView's horizontal scroller.
    pub fn set_has_horizontal_scroller(&self, has: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = downcast::<NSScrollView>(self.ns_view()) {
            s.setHasHorizontalScroller(has);
        }
    }

    /// Show/hide an NSScrollView's vertical scroller.
    pub fn set_has_vertical_scroller(&self, has: bool) {
        use objc2_app_kit::NSScrollView;
        if let Some(s) = downcast::<NSScrollView>(self.ns_view()) {
            s.setHasVerticalScroller(has);
        }
    }

    /// Toggle whether an NSProgressIndicator stays visible when
    /// stopped (vs hiding itself entirely). Useful for spinners
    /// that should reserve space even when idle.
    pub fn set_progress_displayed_when_stopped(&self, shown: bool) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) =
            downcast::<NSProgressIndicator>(self.ns_view())
        {
            p.setDisplayedWhenStopped(shown);
        }
    }

    /// Read the current value of a `<date_picker>`. Returns
    /// `Date::now()` for non-date-picker views.
    pub fn date_picker_value(&self) -> crate::Date {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) =
            downcast::<NSDatePicker>(self.ns_view())
        {
            let d = dp.dateValue();
            return crate::Date::from_nsdate(&d);
        }
        crate::Date::now()
    }

    /// Set the date shown in a `<date_picker>`. No-op on
    /// non-date-picker views.
    pub fn set_date_picker_value(&self, d: crate::Date) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) =
            downcast::<NSDatePicker>(self.ns_view())
        {
            // Diff before mutating: NSDatePicker won't fire its
            // action when the same value is re-set, so this is
            // belt-and-suspenders for bind: cycles.
            let current = dp.dateValue();
            let current_secs = current.timeIntervalSince1970();
            if (current_secs - d.seconds_since_epoch).abs()
                > f64::EPSILON
            {
                dp.setDateValue(&d.to_nsdate());
            }
        }
    }

    /// Read the value of a `<stepper>`. Returns 0.0 for non-stepper
    /// views.
    pub fn stepper_value(&self) -> f64 {
        use objc2_app_kit::NSStepper;
        if let Some(s) =
            downcast::<NSStepper>(self.ns_view())
        {
            return s.doubleValue();
        }
        0.0
    }

    /// Set the value of a `<stepper>`. Diffs first.
    pub fn set_stepper_value(&self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) =
            downcast::<NSStepper>(self.ns_view())
        {
            if (s.doubleValue() - v).abs() > f64::EPSILON {
                s.setDoubleValue(v);
            }
        }
    }

    /// Configure a `<stepper>`'s min, max, and increment in one
    /// call. No-op on non-stepper views.
    pub fn configure_stepper(
        &self,
        min: f64,
        max: f64,
        increment: f64,
    ) {
        use objc2_app_kit::NSStepper;
        if let Some(s) =
            downcast::<NSStepper>(self.ns_view())
        {
            s.setMinValue(min);
            s.setMaxValue(max);
            s.setIncrement(increment);
        }
    }

    /// Set the `value` of a `<progress_indicator>` (0..max).
    /// No-op on non-progress views.
    pub fn set_progress_value(&self, v: f64) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) =
            downcast::<NSProgressIndicator>(self.ns_view())
        {
            p.setDoubleValue(v);
        }
    }

    /// Switch a `<progress_indicator>` between determinate (bar)
    /// and indeterminate (spinner). Indeterminate mode
    /// auto-starts the animation.
    pub fn set_progress_indeterminate(&self, indeterminate: bool) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) =
            downcast::<NSProgressIndicator>(self.ns_view())
        {
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

    /// Set the max value (the upper bound of the bar). Default 1.0.
    pub fn set_progress_max(&self, max: f64) {
        use objc2_app_kit::NSProgressIndicator;
        if let Some(p) =
            downcast::<NSProgressIndicator>(self.ns_view())
        {
            p.setMaxValue(max);
        }
    }

    /// Read the current color from an `<color_well>`. Returns
    /// [`Color::BLACK`] for non-color-well views and for the rare
    /// case where AppKit declines to convert the picker's color
    /// into sRGB.
    pub fn color_well_value(&self) -> crate::Color {
        use objc2_app_kit::NSColorWell;
        if let Some(cw) =
            downcast::<NSColorWell>(self.ns_view())
        {
            let c = cw.color();
            return crate::Color::from_nscolor(&c)
                .unwrap_or(crate::Color::BLACK);
        }
        crate::Color::BLACK
    }

    /// Set the color shown in an `<color_well>`. No-op on non-
    /// color-well views.
    pub fn set_color_well_value(&self, color: crate::Color) {
        use objc2_app_kit::NSColorWell;
        if let Some(cw) =
            downcast::<NSColorWell>(self.ns_view())
        {
            cw.setColor(&color.to_nscolor());
        }
    }

    /// Wire a callback that fires when the text field commits edits
    /// (Return key or focus loss — `controlTextDidEndEditing:`).
    /// Receives the field's current value. No-op on non-NSTextField.
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

    /// Wire a keydown observer on a text field. Fires on
    /// recognized command keys (Enter, Escape, Tab, arrows) routed
    /// through `control:textView:doCommandBySelector:`. No-op on
    /// non-NSTextField. See [`crate::KeyEvent`] for coverage.
    pub fn on_text_keydown(
        &self,
        cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_keydown(field, cb);
        }
    }

    /// Wire a keyup observer on a text field. AppKit's field-
    /// editor command pipeline doesn't distinguish down from up —
    /// this fires on the same notification as `on_text_keydown`.
    /// No-op on non-NSTextField.
    pub fn on_text_keyup(
        &self,
        cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        if let Some(field) = downcast::<NSTextField>(self.ns_view()) {
            crate::event::on_text_field_keyup(field, cb);
        }
    }

    /// Load an image into an `<image_view>` from a file path on
    /// disk. Empty path or load failure clears the image (matching
    /// web `<img src="">` semantics — the view becomes blank rather
    /// than panicking on a bad path).
    pub fn set_image_view_path(&self, path: &str) {
        use objc2_app_kit::{NSImage, NSImageView};
        let Some(iv) = downcast::<NSImageView>(self.ns_view()) else {
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
        crate::layout::schedule_relayout(&self.node);
    }

    /// Wire a change observer on the NSTextView inside a
    /// `<text_view>`. Fires on every keystroke. No-op if this
    /// element isn't a text_view.
    ///
    /// Used by `bind:value` on TextView (write-back leg) and by
    /// future `on:input` support.
    pub fn on_text_view_change(
        &self,
        cb: impl FnMut(String) + 'static,
    ) {
        let view = self.ns_view();
        let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
        else {
            return;
        };
        let Some(doc) = scroll.documentView() else { return };
        let any_doc: &AnyObject = &doc;
        if let Some(tv) =
            any_doc.downcast_ref::<objc2_app_kit::NSTextView>()
        {
            crate::event::on_text_view_change(tv, cb);
        }
    }

    /// Set the editability of the NSTextView inside a `<text_view>`
    /// (which is an NSScrollView wrapping an NSTextView). No-op if
    /// this element isn't a text_view.
    pub fn set_text_view_editable(&self, editable: bool) {
        let view = self.ns_view();
        let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
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

    /// Read the value of a `<text_view>`. Returns `None` for
    /// non-text_view elements; `Some(String)` otherwise. (For
    /// `<text_field>` use the existing NSTextField path —
    /// `text_view` differs because it's wrapped in a scroll view.)
    pub fn text_view_value(&self) -> Option<String> {
        let scroll =
            downcast::<objc2_app_kit::NSScrollView>(self.ns_view())?;
        let doc = scroll.documentView()?;
        let any_doc: &AnyObject = &doc;
        let tv = any_doc.downcast_ref::<objc2_app_kit::NSTextView>()?;
        Some(tv.string().to_string())
    }

    /// Make this element the first responder of its window —
    /// the focus + keyboard target. Web parity: matches
    /// `web_sys::HtmlElement::focus()`.
    ///
    /// No-op if the element isn't mounted in a window. Returns
    /// `true` if AppKit accepted the focus change, `false`
    /// otherwise (e.g. the view declines first-responder status).
    pub fn focus(&self) -> bool {
        let view = self.ns_view();
        let Some(window) = view.window() else { return false };
        // NSView's superclass is NSResponder — `&NSView` derefs
        // through the objc2 class hierarchy.
        let responder: &objc2_app_kit::NSResponder = view;
        window.makeFirstResponder(Some(responder))
    }

    /// Resign first-responder status. Calls
    /// `window.makeFirstResponder(nil)`, which clears the focus.
    /// No-op if the element isn't mounted, or if no view in this
    /// window currently has focus.
    ///
    /// Note: this is a window-wide operation rather than a
    /// view-specific one. AppKit doesn't have a "blur this
    /// specific view" API — calling `resignFirstResponder` on a
    /// view directly only works if the responder chain accepts
    /// the resignation. Going through the window is the
    /// idiomatic clear.
    pub fn blur(&self) -> bool {
        let view = self.ns_view();
        let Some(window) = view.window() else { return false };
        window.makeFirstResponder(None)
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
