//! Node, Element, Text, Placeholder — the DOM-shaped wrappers over
//! `Retained<NSView>`.
//!
//! Each `Node` is a single `Rc<NodeInner>` that carries:
//!   * the tree it lives in (`TreeRef`),
//!   * its arena `NodeId`,
//!   * a cached `Retained<NSView>` for cheap `&NSView` access,
//!   * an `is_borrowed` flag controlling whether `Drop` decrefs the
//!     arena entry.
//!
//! All style / meta / handler state lives in the arena's `NodeData`.
//! Accessors (`with_style`, `with_meta`, `with_handlers_mut`) route
//! straight to the arena. Allocation is eager: `Element::create_<tag>`
//! takes a `tree: &TreeRef` and allocates an arena entry up front.
//! See `crate::layout` for the attach/relayout helpers and
//! `MEMORY_POLICY.md` for the ownership rules.
//!
//! See the crate-level docs for the threading contract.

use crate::layout::{
    CocoaMeta, LayoutHandle, NodeId, Style, TreeRef,
};
use objc2::{
    rc::Retained, runtime::AnyObject, DowncastTarget, MainThreadMarker,
    MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSButton, NSControl, NSTextField, NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use send_wrapper::SendWrapper;
use std::{fmt, rc::Rc};

/// The core node wrapper — a thin handle into a `LayoutTree` arena.
///
/// `Node` is `Clone` (single Rc bump) and `Send + 'static` (via
/// [`SendWrapper`]). Touched only from the main thread; off-main
/// access panics from the SendWrapper runtime check.
///
/// Every Node clone shares **one** `Rc<NodeInner>` carrying:
///   * `(tree, id)` — the arena and the entry id (stable for the
///     entry's lifetime).
///   * `view: Retained<NSView>` — cached so `ns_view() -> &NSView`
///     doesn't touch the arena's RefCell. The arena entry has its
///     own retain too; the two stay in lockstep.
///   * `is_borrowed` — distinguishes the rare "non-owning" Node
///     produced by [`Node::from_view_with_handle`] from a regular
///     owning Node.
///
/// All per-node state (style, meta, handlers) lives in
/// `NodeData<CocoaBackend>` in the arena's slotmap. Accessors
/// (`with_style`, `with_meta`, `with_handlers_mut`) read/write
/// straight through the (tree, id) key.
///
/// When the last clone of an OWNING Node drops, `NodeInner::Drop`
/// calls `tree.decref(id)`. The arena's removal rule (refcount=0
/// AND parent=None) decides whether to actually free the entry —
/// an entry still reachable through a parent's `children` list
/// stays alive even with zero external Node handles. The arena's
/// `NodeData` field-drop order (handlers before view) nils any
/// installed `setTarget` / `setDelegate` while the view is still
/// live, then releases the view.
///
/// For closure-capture patterns that want to refer back to a node
/// from a handler without forming the Element-capture cycle, use
/// [`WeakNode`] / [`WeakElement`].
#[derive(Clone)]
pub struct Node {
    inner: SendWrapper<Rc<NodeInner>>,
}

pub(crate) struct NodeInner {
    /// The arena this node lives in.
    tree: TreeRef,
    /// Stable arena id for this node.
    id: NodeId,
    /// Cached `Retained<NSView>` so `ns_view() -> &NSView` doesn't
    /// need an arena borrow. Adds one ObjC retain per `NodeInner`
    /// (the arena's `NodeData::view` is the other retain) — small
    /// price for keeping the `&NSView` accessor API stable. The
    /// pointer is stable: arena `view` and this one both point at
    /// the same NSView object.
    view: Retained<NSView>,
    /// When true, `Drop` does NOT decref the arena entry — this
    /// Node is just a borrowed view onto someone else's entry.
    /// Used by [`Node::from_view_with_handle`] (synthesised parent
    /// wrappers + deferred-relayout transient roots).
    is_borrowed: bool,
}

impl Drop for NodeInner {
    fn drop(&mut self) {
        if !self.is_borrowed {
            // Decref. The arena's removal rule (refcount=0 AND
            // parent=None) decides whether to actually drop the
            // entry. If there's still a parent attachment, the
            // entry stays alive under reachability.
            //
            // The arena's NodeData field-drop order (handlers
            // before view) handles ObjC delegate nilling while
            // the view is still live.
            //
            // Scroll-view documentView wrappers are now allocated
            // via `tree.new_internal_leaf` (refcount=0) — they're
            // collected automatically by the transitive reachability
            // sweep in `tree.remove` once their parent (this node)
            // goes away. No explicit removal needed here.
            self.tree.decref(self.id);
        }
    }
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr: *const NSView = &*self.inner.view;
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
    /// Allocate a fresh arena entry into `tree` and return a Node
    /// owning it. The view is retained twice — once on `NodeInner`
    /// for fast `&NSView` access, once in the arena `NodeData::view`
    /// for layout / drop ordering.
    ///
    /// The typed registration primitive: hand in a concrete NSView
    /// subclass, get back a `Node` owning a fresh arena entry. Used
    /// by every typed-builder construction path in `leptos_cocoa`
    /// (e.g. each builder allocates its own NSButton / NSTextField /
    /// NSScrollView, then calls `Node::from_view`), and by the
    /// renderer-protocol primitives [`Element::create_text`] and
    /// [`Element::create_placeholder`].
    pub fn from_view<V>(
        tree: &TreeRef,
        view: Retained<V>,
        default_style: Style,
        default_meta: CocoaMeta,
    ) -> Self
    where
        V: AsRef<NSView> + Message,
    {
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(view) };
        let view_for_arena = view.clone();
        let id = tree.new_leaf(
            default_style,
            SendWrapper::new(view_for_arena),
            default_meta,
            crate::event::NodeHandlers::default(),
        );
        // Wire the handlers' view back-ref so Drop can nil
        // setTarget/setDelegate while the view is still alive.
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
    /// [`LayoutHandle`] — i.e. one that already references a Taffy
    /// node in some tree. The resulting Node is **borrowed**: its
    /// `Drop` does NOT remove the arena entry, because some other
    /// Node (the original) owns it. Used by `mount_before` in
    /// `tachys::renderer::cocoa::Dom` to synthesise a parent
    /// Element wrapper for an NSView whose Node we don't have, and
    /// by the deferred-relayout path to manufacture a transient
    /// root Node.
    pub fn from_view_with_handle<V>(
        view: Retained<V>,
        handle: LayoutHandle,
    ) -> Self
    where
        V: AsRef<NSView> + Message,
    {
        let view: Retained<NSView> = unsafe { Retained::cast_unchecked(view) };
        let inner = NodeInner {
            tree: handle.tree,
            id: handle.node_id,
            view,
            is_borrowed: true,
        };
        Node { inner: SendWrapper::new(Rc::new(inner)) }
    }

    /// Borrow the underlying NSView. Main-thread only.
    pub fn ns_view(&self) -> &NSView {
        &self.inner.view
    }

    /// Get a new `Retained<NSView>` (sends `retain`) without
    /// cloning the whole Node. Use this in callback captures
    /// where you need to message the view later but don't want to
    /// pull the Rust `Node` clones into the capture — capturing
    /// `Element` / `Node` inside a closure stored on this same
    /// Node's handlers would form a cycle (closure → Element →
    /// Node → handlers → closure).
    pub fn ns_view_retained(&self) -> Retained<NSView> {
        self.inner.view.clone()
    }

    /// Pointer-equality check (same underlying NSView object).
    pub fn ptr_eq(&self, other: &Node) -> bool {
        let a: *const NSView = &*self.inner.view;
        let b: *const NSView = &*other.inner.view;
        a == b
    }

    /// Drop the resources owned by this node and detach it from its
    /// superview. Removes the arena entry eagerly (bypasses the
    /// refcount-still-positive reachability check), so node counts
    /// return to baseline immediately on unmount. Subsequent
    /// accessors on this Node will see a missing arena entry and
    /// silently no-op.
    pub fn teardown(&self) {
        // `tree.remove` recursively GCs orphaned children with
        // refcount=0 (the scroll-view wrapper case), so we don't
        // need to remove it explicitly here.
        self.inner.tree.remove(self.inner.id);
        self.ns_view().removeFromSuperview();
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

    /// Borrow the node's [`CocoaMeta`] for read.
    pub fn with_meta<R>(&self, f: impl FnOnce(&CocoaMeta) -> R) -> R {
        let meta = self.inner.tree.meta(self.inner.id).unwrap_or_default();
        f(&meta)
    }

    /// Mutate the node's [`CocoaMeta`]. Pushed back into the tree.
    pub fn with_meta_mut<R>(&self, f: impl FnOnce(&mut CocoaMeta) -> R) -> R {
        let mut meta = self.inner.tree.meta(self.inner.id).unwrap_or_default();
        let r = f(&mut meta);
        self.inner.tree.set_meta(self.inner.id, meta);
        r
    }

    /// Mutate this node's per-node handler set in the arena. Used
    /// by `cocoa_dom::event::on_*` install helpers.
    pub fn with_handlers_mut<R>(
        &self,
        f: impl FnOnce(&mut crate::event::NodeHandlers) -> R,
    ) -> R {
        self.inner
            .tree
            .with_handlers_mut(self.inner.id, f)
            .expect("Node id must exist in arena")
    }

    /// Returns the `(TreeRef, NodeId)` pair. Always `Some` now that
    /// every Node has a tree from creation — the `Option` is kept
    /// for API stability with existing call sites.
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

    /// Test-only: number of strong refs to the inner Rc (proxy for
    /// "how many Node clones exist").
    #[doc(hidden)]
    pub fn handlers_rc_count_for_test(&self) -> usize {
        Rc::strong_count(&*self.inner)
    }
}

// ---------------------------------------------------------------------
// Node — typed-builder / renderer-protocol surface
// ---------------------------------------------------------------------

/// Backwards-compatibility alias. Element used to be a distinct wrapper
/// over `Node`; after the kind-discriminant + Text/Placeholder
/// unification, the wrapper had no remaining state. The two were
/// merged: `Node` is now the single user-facing type for every
/// AppKit-backed arena entry.
pub type Element = Node;

impl Node {
    /// Identity. Kept (along with [`Self::into_node`] and
    /// [`Self::from_node_unchecked`]) so the pre-unification call
    /// style `el.as_node()` / `el.into_node()` /
    /// `Element::from_node_unchecked(n)` keeps working. New code can
    /// just use the Node directly.
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

    /// Generic flipped container (FlippedView, default Taffy style).
    /// Used by `cocoa_dom::window` / `cocoa_dom::split_window` for
    /// the content root, and by leptos_cocoa's Stack / view builders.
    pub fn create_container(tree: &TreeRef) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_container_with(tree, mtm)
    }

    pub fn create_container_with(tree: &TreeRef, mtm: MainThreadMarker) -> Self {
        use crate::{flipped_view::FlippedView, layout::Style};
        let view: Retained<NSView> = unsafe {
            Retained::cast_unchecked(FlippedView::new(mtm))
        };
        Node::from_view(tree, view, Style::default(), CocoaMeta::default())
    }

    /// The NSView that *actually* parents this node's children.
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
        let routes_to_doc = self.with_meta(|m| m.is_scroll_view);
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
                //
                // The debug overlay (when the `debug-overlay` feature
                // is on) lives in the subview list but isn't a Taffy
                // child, so we skip it here — otherwise the Taffy
                // index would be off by one for every child added
                // while the overlay is installed.
                let subviews = parent.subviews();
                let child_ptr: *const NSView = child_view;
                let mut child_index = 0_usize;
                for sv in subviews.iter() {
                    let sv_ptr: *const NSView = &*sv;
                    if sv_ptr == child_ptr {
                        break;
                    }
                    #[cfg(feature = "debug-overlay")]
                    {
                        if sv.tag() == crate::debug_overlay::OVERLAY_TAG {
                            continue;
                        }
                    }
                    child_index += 1;
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

    /// Set the title on an NSButton (push button / checkbox).
    /// No-op on other view classes. Diffs first so a same-value set
    /// doesn't trigger an unnecessary layout/redraw — important for
    /// `bind:` effects that re-fire with unchanged values.
    pub fn set_title(&self, value: &str) {
        let view = self.ns_view();
        if let Some(button) = downcast::<NSButton>(view) {
            let current = button.title().to_string();
            if current != value {
                button.setTitle(&NSString::from_str(value));
                crate::layout::schedule_relayout(self);
            }
        }
    }

    /// Set the string value on an NSControl (`setStringValue:`) or,
    /// for the `<text_view>` wrapper (NSScrollView → NSTextView),
    /// route through the documentView's `setString:`. No-op on other
    /// view classes.
    pub fn set_value(&self, value: &str) {
        let view = self.ns_view();
        if let Some(control) = downcast::<NSControl>(view) {
            let current = control.stringValue().to_string();
            if current != value {
                control.setStringValue(&NSString::from_str(value));
                crate::layout::schedule_relayout(self);
            }
        } else if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
        {
            if let Some(doc) = scroll.documentView() {
                let any_doc: &objc2::runtime::AnyObject = &doc;
                if let Some(tv) =
                    any_doc.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    let current = tv.string().to_string();
                    if current != value {
                        tv.setString(&NSString::from_str(value));
                        crate::layout::schedule_relayout(self);
                    }
                }
            }
        }
    }

    /// Set the placeholder string on an NSTextField. No-op on other
    /// view classes. The placeholder shows when the field is empty;
    /// its width contributes to `intrinsicContentSize`, so a change
    /// schedules a relayout.
    pub fn set_placeholder(&self, value: &str) {
        let view = self.ns_view();
        if let Some(field) = downcast::<NSTextField>(view) {
            let current: String = field
                .placeholderString()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if current != value {
                field.setPlaceholderString(Some(&NSString::from_str(value)));
                crate::layout::schedule_relayout(self);
            }
        }
    }

    /// Toggle the underlying NSView's visibility. Diffs first to
    /// avoid redundant redraws.
    pub fn set_hidden(&self, value: bool) {
        let view = self.ns_view();
        if view.isHidden() != value {
            view.setHidden(value);
        }
    }

    /// Toggle the enabled state on an NSControl. No-op on plain
    /// NSViews. Diffs first to avoid focus-ring flashes.
    pub fn set_enabled(&self, value: bool) {
        let view = self.ns_view();
        if let Some(control) = downcast::<NSControl>(view) {
            if control.isEnabled() != value {
                control.setEnabled(value);
            }
        }
    }

    /// Set the on/off state on an NSButton (checkbox / radio).
    /// No-op on other view classes.
    pub fn set_checked(&self, value: bool) {
        let view = self.ns_view();
        if let Some(button) = downcast::<NSButton>(view) {
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

    /// Wire a click handler to this element. No-op if this element
    /// isn't an NSButton-class instance — silently dropped, matching
    /// the web `addEventListener` shape.
    ///
    /// Limitation: each call replaces NSControl's stored target/action
    /// pair, so calling twice keeps the latest handler only.
    /// (Multiple-listener support will need a fan-out target that
    /// holds a Vec<Box<dyn FnMut>>.)
    pub fn on_click(&self, cb: impl FnMut() + 'static) {
        // NSButton is an NSControl, so on_control_action covers it;
        // the inner fn no-ops on non-NSControl nodes.
        crate::event::on_control_action(self, cb);
    }

    /// Wire a callback that fires when an NSControl's value changes
    /// (slider drag, popup selection, button click — any
    /// target/action). No-op if this element isn't an NSControl.
    ///
    /// This is the generic version of [`on_click`]; use it for
    /// slider/popup-style controls where "click" is misleading.
    pub fn on_action(&self, cb: impl FnMut() + 'static) {
        crate::event::on_control_action(self, cb);
    }

    /// Wire a unit-payload callback that fires whenever a control's
    /// value changes — every keystroke for text fields (delegate
    /// based; supports multiple handlers); every drag tick for
    /// sliders / steppers / etc. (NSControl target/action; single
    /// handler). No-op for elements that aren't value-bearing.
    pub fn on_value_change(&self, mut cb: impl FnMut() + Send + 'static) {
        // Text fields go through the delegate fan-out so we can
        // stack multiple handlers (matches on_text_change's pattern).
        if downcast::<NSTextField>(self.ns_view()).is_some() {
            crate::event::on_text_field_change(self, move |_| cb());
            return;
        }
        crate::event::on_control_action(self, cb);
    }

    /// Wire a callback that fires whenever the text content of a
    /// text-field changes (every keystroke / paste / etc.). No-op
    /// if this element isn't an NSTextField. Multiple handlers are
    /// supported — each call appends to the field's fan-out
    /// delegate.
    pub fn on_text_change(&self, cb: impl FnMut(String) + 'static) {
        crate::event::on_text_field_change(self, cb);
    }

    /// Install hover tracking. `cb(true)` fires when the cursor
    /// enters the element's visible rect; `cb(false)` when it
    /// exits. Single handler per element — combine into one
    /// closure if you need to fan out.
    pub fn on_hover(&self, cb: impl FnMut(bool) + 'static) {
        crate::event::on_hover(self, cb);
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
        let old = v.alphaValue();
        if (old - clamped).abs() <= f64::EPSILON {
            return;
        }
        #[cfg(feature = "animation")]
        let visible_opacity = {
            v.setWantsLayer(true);
            v.layer().map(|layer| {
                crate::animation::presentation_or_model(
                    &layer, |l| l.opacity() as f64,
                )
            })
        };
        v.setAlphaValue(clamped);
        #[cfg(feature = "animation")]
        if let (Some(visible), Some(layer)) = (visible_opacity, v.layer()) {
            crate::animation::animate_float(
                &layer, "opacity", visible, clamped,
            );
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
    pub fn set_text_alignment(&self, alignment: crate::TextAlignment) {
        let view = self.ns_view();

        if let Some(field) = downcast::<NSTextField>(view) {
            field.setAlignment(alignment.0);
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
                    tv.setAlignment(alignment.0);
                }
            }
        }
    }

    /// Set the font size (in points) on a text-bearing view.
    /// **Preserves every symbolic trait** on the existing font —
    /// bold, italic, condensed, expanded, monospace, vertical, etc.
    /// Only the point size changes. No-op on non-text views.
    pub fn set_font_size(&self, points: f64) {
        let traits = self.read_font_traits();
        self.apply_font(points, traits);
    }

    /// Toggle bold weight on a text-bearing view. Preserves the
    /// current point size and every other symbolic trait.
    pub fn set_bold(&self, bold: bool) {
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

    /// Toggle italic on a text-bearing view. Preserves point size
    /// and other symbolic traits.
    pub fn set_italic(&self, italic: bool) {
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

    /// Build a font at `points` carrying `traits` (bold, italic,
    /// condensed, etc.) and install it on whichever text-bearing
    /// widget this view backs. Uses
    /// `NSFontDescriptor.fontDescriptorWithSymbolicTraits:` so the
    /// trait bits go through unchanged — no NSFontManager
    /// translation layer.
    fn apply_font(
        &self,
        points: f64,
        traits: objc2_app_kit::NSFontDescriptorSymbolicTraits,
    ) {
        use objc2_app_kit::NSFont;
        let plain = NSFont::systemFontOfSize(points);
        // Start from the plain system-font descriptor; layer the
        // requested symbolic traits on. The resulting descriptor
        // may resolve to a substitute font (e.g. SF Mono for the
        // MonoSpace trait); we only use it if the new descriptor
        // can be turned back into a concrete NSFont. Otherwise
        // fall back to the plain font — better than a panic.
        let font = if traits.is_empty() {
            plain
        } else {
            let base = plain.fontDescriptor();
            let with_traits = base.fontDescriptorWithSymbolicTraits(traits);
            NSFont::fontWithDescriptor_size(&with_traits, points)
                .unwrap_or(plain)
        };

        let view = self.ns_view();
        if let Some(field) = downcast::<NSTextField>(view) {
            field.setFont(Some(&font));
        } else if let Some(button) = downcast::<NSButton>(view) {
            button.setFont(Some(&font));
        } else if let Some(scroll) =
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
        crate::layout::schedule_relayout(self);
    }

    fn read_font_point_size(&self) -> Option<f64> {
        let view = self.ns_view();
        if let Some(field) = downcast::<NSTextField>(view) {
            return field.font().map(|f| f.pointSize());
        }
        if let Some(button) = downcast::<NSButton>(view) {
            return button.font().map(|f| f.pointSize());
        }
        None
    }

    /// Read the symbolic traits (bold, italic, condensed, ...) of
    /// the view's current font. Returns the empty bitset when there
    /// is no font (non-text view), or when no traits are active.
    fn read_font_traits(&self) -> objc2_app_kit::NSFontDescriptorSymbolicTraits {
        use objc2_app_kit::NSFontDescriptorSymbolicTraits;
        let view = self.ns_view();
        let font = if let Some(field) = downcast::<NSTextField>(view) {
            field.font()
        } else if let Some(button) = downcast::<NSButton>(view) {
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

    /// Apply a custom title color to an NSButton via the
    /// `contentTintColor` property. Recolors both the title text
    /// and any template-image bitmaps; no `attributedTitle`
    /// round-trip required. No-op on non-buttons.
    ///
    /// **macOS deployment target**: `contentTintColor` was added in
    /// macOS 10.14 (Mojave). The crate's effective MSRV is whatever
    /// `objc2-app-kit` requires (currently 10.13). Pre-10.14
    /// targets would `respondsToSelector:` away the call — we
    /// don't currently guard it. If you need pre-Mojave support,
    /// drop back to a manually-applied `attributedTitle`.
    pub fn set_button_title_color(&self, color: crate::Color) {
        let Some(button) = downcast::<NSButton>(self.ns_view()) else {
            return;
        };
        let ns_color = color.to_nscolor();
        button.setContentTintColor(Some(&ns_color));
    }

    /// Render an SF Symbol as the button's image. Empty name clears
    /// the image. macOS 11+; older systems render nothing for this
    /// slot.
    ///
    /// Image position is chosen by whether a title is set:
    ///   * no title  → `ImageOnly` (icon-only button)
    ///   * has title → `ImageLeading` (icon to the left of the title)
    ///
    /// Both render reliably with the default `Push` bezel. The
    /// classic "icon above caption" toolbar layout doesn't have a
    /// dependable NSButton bezel — use the native `<toolbar>` +
    /// `<toolbar_item>` elements (built on `NSToolbar`) instead.
    pub fn set_button_sf_symbol(&self, name: &str) {
        use objc2_app_kit::NSCellImagePosition;
        let Some(button) = downcast::<NSButton>(self.ns_view()) else {
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
        crate::layout::schedule_relayout(self);
    }

    /// Toggle the `intrinsic_width = FromContent` opt-in on an
    /// editable NSTextField. With `true`, the field's measured width
    /// follows its content (NSTextField's natural behaviour); with
    /// `false` (the default), the measure pass returns width=0 so
    /// the parent decides via cross-axis stretch / flex_grow. No-op
    /// on non-NSTextField.
    ///
    /// Use the `intrinsic_width` builder method on `<text_field>`
    /// to configure this declaratively.
    pub fn set_intrinsic_width_from_content(&self, on: bool) {
        if downcast::<NSTextField>(self.ns_view()).is_some() {
            crate::layout::mark_intrinsic_width_from_content(
                self,
                on,
            );
            crate::layout::schedule_relayout(self);
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

    /// Set the line-break behaviour on a text view.
    /// `LineBreak::WORD_WRAP` / `CHAR_WRAP` allow wrapping;
    /// `TRUNCATE_HEAD/TAIL/MIDDLE` keep one line with an ellipsis;
    /// `CLIP` truncates silently with no indicator. Supports
    /// **both** NSTextField (labels, text fields) and NSTextView
    /// (scroll_view-wrapped multi-line editor). No-op on other
    /// view kinds.
    pub fn set_line_break(&self, mode: crate::LineBreak) {
        use objc2_app_kit::NSLineBreakMode;
        let wraps = matches!(
            mode.0,
            NSLineBreakMode::ByWordWrapping | NSLineBreakMode::ByCharWrapping
        );
        let view = self.ns_view();
        if let Some(f) = downcast::<NSTextField>(view) {
            f.setUsesSingleLineMode(!wraps);
            f.cell()
                .expect("NSTextField always has a cell")
                .setLineBreakMode(mode.0);
            // 0 = "as many as needed"; the truncate modes are
            // effectively single-line via setUsesSingleLineMode.
            f.setMaximumNumberOfLines(0);
            crate::layout::schedule_relayout(self);
            return;
        }
        // NSTextView lives inside an NSScrollView (our <text_view>
        // tag). Its text-container governs line breaking.
        if let Some(scroll) =
            downcast::<objc2_app_kit::NSScrollView>(view)
        {
            if let Some(doc) = scroll.documentView() {
                let any: &AnyObject = &doc;
                if let Some(tv) =
                    any.downcast_ref::<objc2_app_kit::NSTextView>()
                {
                    // SAFETY: textContainer is unsafe in objc2 because
                    // the returned pointer could in principle be
                    // null; for an NSTextView that has been added to
                    // the view hierarchy (which all our text_views
                    // are at this point), the container is always
                    // present. Treat None as "no-op" defensively.
                    if let Some(container) =
                        unsafe { tv.textContainer() }
                    {
                        container.setLineBreakMode(mode.0);
                        // Truncation modes need a finite width and
                        // unbounded height to surface the ellipsis;
                        // wrapping modes already have that shape by
                        // default. Keep the existing geometry — we
                        // only touch line-break mode.
                    }
                    crate::layout::schedule_relayout(self);
                }
            }
        }
    }

    /// Shorthand for `set_line_break(ByWordWrapping/ByTruncatingTail)`
    /// — kept for source-compat with the older `Label::multiline(true)`
    /// builder method. New call sites should prefer
    /// `set_line_break` directly.
    pub fn set_multiline(&self, multi: bool) {
        self.set_line_break(if multi {
            crate::LineBreak::WORD_WRAP
        } else {
            crate::LineBreak::TRUNCATE_TAIL
        });
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

    /// Set an NSSegmentedControl's visual style. No-op on
    /// non-segmented.
    pub fn set_segment_style(&self, style: crate::SegmentStyle) {
        use objc2_app_kit::NSSegmentedControl;
        if let Some(sc) =
            downcast::<NSSegmentedControl>(self.ns_view())
        {
            sc.setSegmentStyle(style.0);
        }
    }

    /// Set NSDatePicker's visual style (textual / textual+stepper /
    /// clock-and-calendar). No-op on non-date-pickers.
    pub fn set_date_picker_style(&self, style: crate::DatePickerStyle) {
        use objc2_app_kit::NSDatePicker;
        if let Some(dp) = downcast::<NSDatePicker>(self.ns_view()) {
            dp.setDatePickerStyle(style.0);
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

    /// Configure an `<scroll_view>`'s scroll axis. Stashes the
    /// choice on the Node's meta (read by
    /// `cocoa_dom::layout::set_as_root` when allocating the
    /// documentView wrapper) and sets sensible scroller-visibility
    /// defaults for the chosen axis. The explicit
    /// `set_has_*_scroller` setters can still be used to override
    /// the defaults afterward.
    ///
    /// Must be called before the element joins a layout tree —
    /// builder code calls this from `Render::build` between
    /// `Element::create_scroll_view` and the first mount.
    /// No-op on non-scroll-view elements.
    pub fn set_scroll_axis(&self, axis: crate::layout::ScrollAxis) {
        use crate::layout::ScrollAxis;
        use taffy::FlexDirection;
        let node = self.as_node();
        if !node.with_meta(|m| m.is_scroll_view) {
            return;
        }
        node.with_meta_mut(|m| m.scroll_axis = axis);

        // The scroll_view's own `flex_direction` determines the
        // wrapper's main axis (the wrapper is its only Taffy child),
        // which in turn determines what the wrapper's `flex_shrink: 0`
        // protects from squashing. Flip Row for Horizontal so
        // `flex_shrink: 0` keeps the wrapper horizontally rigid.
        //
        // We deliberately **don't** touch `flex_basis: 0` or
        // `min_size.height: 0` here. Those defaults are what prevent
        // the scroll_view's content from inflating ancestor flex
        // containers via intrinsic sizing — see the comment in
        // `Element::create_<tag>` for "scroll_view". To size a
        // horizontal scroll_view, use `min_height` (or `flex_grow`
        // with a bounded parent), the same way vertical scroll_views
        // size today. `height=N` alone won't work on a scroll_view
        // because `flex_basis: 0` overrides `size.height` in the
        // flex algorithm.
        node.with_style_mut(|s| {
            s.flex_direction = match axis {
                ScrollAxis::Vertical | ScrollAxis::Both => FlexDirection::Column,
                ScrollAxis::Horizontal => FlexDirection::Row,
            };
        });

        // Rewrite the documentView wrapper's Taffy style so the new
        // axis takes effect. Wrapper id was stashed on meta at
        // `Element::create_<tag>` time.
        let wrapper = node.with_meta(|m| m.child_taffy_parent);
        if let (Some(wid), Some((tree, _))) = (wrapper, node.tree_id()) {
            tree.set_style(wid, crate::layout::build_scroll_wrapper_style(axis));
        }

        // Apply scroller-visibility defaults appropriate for the
        // axis. The user's explicit `has_*_scroller` setters run
        // after this in the builder pipeline and can override.
        use objc2_app_kit::NSScrollView;
        if let Some(s) = downcast::<NSScrollView>(self.ns_view()) {
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

    /// Set a `<stepper>`'s min. No-op on non-stepper views.
    pub fn set_stepper_min(&self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = downcast::<NSStepper>(self.ns_view()) {
            s.setMinValue(v);
        }
    }

    /// Set a `<stepper>`'s max. No-op on non-stepper views.
    pub fn set_stepper_max(&self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = downcast::<NSStepper>(self.ns_view()) {
            s.setMaxValue(v);
        }
    }

    /// Set a `<stepper>`'s increment. No-op on non-stepper views.
    pub fn set_stepper_increment(&self, v: f64) {
        use objc2_app_kit::NSStepper;
        if let Some(s) = downcast::<NSStepper>(self.ns_view()) {
            s.setIncrement(v);
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
        crate::event::on_text_field_end_editing(self, cb);
    }

    /// Wire a callback that fires when the text field gains focus
    /// (`controlTextDidBeginEditing:`). No-op on non-NSTextField.
    pub fn on_text_focus(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_focus(self, cb);
    }

    /// Wire a callback that fires when the text field loses focus
    /// (Return / Tab / click-elsewhere — same notification as
    /// `on_text_end_editing` but with no value payload). No-op
    /// on non-NSTextField.
    pub fn on_text_blur(&self, cb: impl FnMut() + 'static) {
        crate::event::on_text_field_blur(self, cb);
    }

    /// Wire a keydown observer on a text field. Fires on
    /// recognized command keys (Enter, Escape, Tab, arrows) routed
    /// through `control:textView:doCommandBySelector:`. No-op on
    /// non-NSTextField. See [`crate::KeyEvent`] for coverage.
    pub fn on_text_keydown(
        &self,
        cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        crate::event::on_text_field_keydown(self, cb);
    }

    /// Wire a keyup observer on a text field. AppKit's field-
    /// editor command pipeline doesn't distinguish down from up —
    /// this fires on the same notification as `on_text_keydown`.
    /// No-op on non-NSTextField.
    pub fn on_text_keyup(
        &self,
        cb: impl FnMut(crate::KeyEvent) + 'static,
    ) {
        crate::event::on_text_field_keyup(self, cb);
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
        crate::layout::schedule_relayout(self);
    }

    /// Load an image into an `<image_view>` from in-memory bytes.
    /// `None` or an empty slice clears the image; data that AppKit
    /// can't decode also clears it (matching `set_image_view_path`'s
    /// no-panic semantics). NSImage's `initWithData:` auto-detects
    /// PNG, JPEG, GIF, TIFF, HEIC, PDF.
    ///
    /// Typical use: HTTP fetch the bytes on a background thread,
    /// hand them to a signal via the async-bridge pattern, then this
    /// reactive setter fires on the main thread.
    pub fn set_image_view_bytes(&self, bytes: Option<&[u8]>) {
        use objc2_app_kit::{NSImage, NSImageView};
        use objc2_foundation::NSData;
        let Some(iv) = downcast::<NSImageView>(self.ns_view()) else {
            return;
        };
        let Some(bytes) = bytes.filter(|b| !b.is_empty()) else {
            iv.setImage(None);
            crate::layout::schedule_relayout(self);
            return;
        };
        use objc2::AllocAnyThread;
        let data = NSData::with_bytes(bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data);
        iv.setImage(image.as_deref());
        crate::layout::schedule_relayout(self);
    }

    /// Set an `<image_view>` to render an SF Symbol by name (e.g.
    /// `"plus.circle"`, `"trash"`, `"square.and.arrow.up"`). Empty
    /// name or an unknown symbol clears the image.
    ///
    /// Requires macOS 11+ (SF Symbols ship with the system from Big
    /// Sur on); on older systems the AppKit call returns nil and the
    /// view stays blank.
    ///
    /// SF Symbols are *template* images — they pick up their tint
    /// from `NSImageView.contentTintColor` (or the enclosing
    /// `contentTintColor` if used in a button). Use `text_color=`
    /// on the image view to drive the tint reactively.
    pub fn set_image_view_sf_symbol(&self, name: &str) {
        use objc2_app_kit::NSImageView;
        let Some(iv) = downcast::<NSImageView>(self.ns_view()) else {
            return;
        };
        let image = sf_symbol_image(name);
        iv.setImage(image.as_deref());
        crate::layout::schedule_relayout(self);
    }

    /// Set an image view's tint color. Applied via
    /// `NSImageView.contentTintColor`, which most heavily affects
    /// template images (SF Symbols). Regular RGBA images render
    /// unchanged.
    pub fn set_image_view_tint(&self, color: crate::Color) {
        use objc2_app_kit::NSImageView;
        let Some(iv) = downcast::<NSImageView>(self.ns_view()) else {
            return;
        };
        iv.setContentTintColor(Some(&color.to_nscolor()));
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
        // The Node's ns_view is the NSScrollView; the install fn
        // pulls out the inner NSTextView and stashes the delegate
        // on this Node's handlers so its lifecycle tracks the
        // scroll-view Node (not the documentView, which has no
        // Rust wrapper).
        crate::event::on_text_view_change(self, cb);
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

}

// ---------------------------------------------------------------------
// Node: text-label & placeholder constructors
// ---------------------------------------------------------------------

impl Node {
    /// Build a text-label Element — a non-editable, non-bordered
    /// NSTextField (AppKit's standard "label" configuration).
    ///
    /// Used by the renderer's `create_text_node`, which is the
    /// `Render` impl for `&str` / `String` / numerics.
    pub fn create_text(tree: &TreeRef, content: &str) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_text_with(tree, content, mtm)
    }

    pub fn create_text_with(
        tree: &TreeRef,
        content: &str,
        mtm: MainThreadMarker,
    ) -> Self {
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

        Node::from_view(tree, view, style, CocoaMeta::default())
    }

    /// Update the displayed string on a text-label Node. No-op if
    /// the backing view isn't an NSTextField.
    pub fn set_text(&self, content: &str) {
        let view = self.ns_view();
        if let Some(field) = downcast::<NSTextField>(view) {
            field.setStringValue(&NSString::from_str(content));
        }
        // Content changed → intrinsic size may have changed too.
        crate::layout::schedule_relayout(self);
    }

    /// Build a placeholder Element — a hidden, zero-sized NSView used
    /// by the renderer's control-flow primitives (`Render for ()`,
    /// tuple/iterator/keyed end-markers) as a stable mount anchor.
    ///
    /// Default Taffy style is `position: absolute; size: 0×0` so the
    /// marker doesn't take a slot in the parent's flex layout —
    /// without that, every empty `()` would offset its siblings by
    /// `gap`.
    pub fn create_placeholder(tree: &TreeRef) -> Self {
        let mtm = MainThreadMarker::new()
            .expect("cocoa_dom must run on the main thread");
        Self::create_placeholder_with(tree, mtm)
    }

    pub fn create_placeholder_with(
        tree: &TreeRef,
        mtm: MainThreadMarker,
    ) -> Self {
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0)),
        );
        view.setHidden(true);

        let mut style = crate::layout::Style::default();
        style.position = crate::layout::Position::Absolute;
        style.size.width = crate::layout::Dimension::length(0.0);
        style.size.height = crate::layout::Dimension::length(0.0);

        Node::from_view(tree, view, style, CocoaMeta::default())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// Best-effort downcast of an `&NSView` to a more specific subclass.
/// Returns `None` if the runtime class isn't a subclass of `T`.
pub(crate) fn downcast<T>(view: &NSView) -> Option<&T>
where
    T: DowncastTarget,
{
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<T>()
}

/// Load an SF Symbol by name and apply a default point-size /
/// weight configuration. Required because SF Symbols ship without a
/// configuration — the raw `NSImage` has zero intrinsic size,
/// which breaks `NSImageView` measurement and `NSButton`/
/// `NSToolbarItem` image rendering.
///
/// Returns `None` if the symbol name isn't recognised (the lookup
/// returns nil on macOS 10.x or for unknown symbol names).
pub(crate) fn sf_symbol_image(
    name: &str,
) -> Option<objc2::rc::Retained<objc2_app_kit::NSImage>> {
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
    // 16pt regular — the standard NSToolbarItem icon size, and a
    // reasonable default for buttons and image views. Callers
    // wanting a different size can post-configure with their own
    // `imageWithSymbolConfiguration:` call.
    let cfg = unsafe {
        NSImageSymbolConfiguration::configurationWithPointSize_weight(
            16.0,
            NSFontWeightRegular,
        )
    };
    raw.imageWithSymbolConfiguration(&cfg).or(Some(raw))
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

// ---------------------------------------------------------------------
// Weak handles — non-owning references for cycle-safe closure capture
// ---------------------------------------------------------------------
//
// The Element-capture cycle: a closure stored in `NodeHandlers` (via
// `Element::on_click(move |...| { node.do_something(); })`) that
// captures a strong `Node`/`Element` clone creates an unbreakable
// cycle:
//
//   closure → captured Element → Rc<NodeInner> → (tree, id)
//     → arena entry refcount > 0 → handlers stay alive
//     → Retained<ActionTarget> stays alive → ivars stay alive
//     → closure stays alive → captured Element stays alive
//
// The cycle keeps the arena entry alive forever; `NodeInner::Drop`
// never fires, `setTarget`/`setDelegate` are never nilled, and the
// fuzzer flags it as a leak. `MEMORY_POLICY.md` §3 prohibits this
// pattern.
//
// `WeakNode` / `WeakElement` are the safe alternative. They hold a
// `Weak<NodeInner>` — non-owning — and
// expose `.upgrade() -> Option<...>` to recover a strong handle at
// fire time:
//
//   ```rust
//   let weak = el.weak();
//   el.on_click(move || {
//       if let Some(el) = weak.upgrade() {
//           el.do_something();
//       }
//   });
//   ```
//
// The closure captures `weak` (no strong Rc bump). When all "real"
// Element clones drop, `NodeInner::Drop` fires normally, handlers
// drop, the closure drops, and any `WeakElement` still inside it
// becomes dangling but harmless (`upgrade()` returns `None`).
//
// Library code (bind.rs, RenderEffect installs) is already cycle-safe
// because its closures live in `ElementState::_effects`, NOT in the
// arena's handlers. These weak types exist so user code that DOES
// want to re-enter the node from a handler can do so cycle-safely.

/// Non-owning weak reference to a `Node`. Use [`Self::upgrade`] to
/// recover a strong `Node` if the original is still alive. Copy is
/// not derivable (the inner `SendWrapper` isn't `Copy`); cloning is
/// cheap (a `Weak::clone`).
#[derive(Clone)]
pub struct WeakNode {
    inner: SendWrapper<std::rc::Weak<NodeInner>>,
}

impl Node {
    /// Get a non-owning weak handle for cycle-safe closure capture.
    /// See the module-level "Weak handles" docs for why and when.
    pub fn downgrade(&self) -> WeakNode {
        WeakNode {
            inner: SendWrapper::new(Rc::downgrade(&*self.inner)),
        }
    }
}

impl WeakNode {
    /// Try to recover a strong `Node`. Returns `None` if all strong
    /// references have dropped (the arena entry may or may not still
    /// exist — that's controlled by parent-reachability).
    pub fn upgrade(&self) -> Option<Node> {
        self.inner
            .upgrade()
            .map(|rc| Node { inner: SendWrapper::new(rc) })
    }

    /// Same as `upgrade().is_some()` but avoids the Rc clone.
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

// ---- WeakElement ----------------------------------------------------

/// Backwards-compatibility alias — see [`Element`] / [`WeakNode`].
pub type WeakElement = WeakNode;

impl Node {
    /// Convenience alias for [`Node::downgrade`] — historical name
    /// from when `Element` was a distinct wrapper.
    pub fn weak(&self) -> WeakNode {
        self.downgrade()
    }
}
