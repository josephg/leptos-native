//! Backend-agnostic layout engine over a **thread-local node store**.
//!
//! Every node lives in a single per-thread [`LayoutState<B>`] slotmap,
//! reached through [`Backend::with_tree`]. A node reference is a
//! bare [`NodeId`] — `Copy + Send`, no handle, no refcount. Stale ids
//! resolve to `None`/no-op via the generational slotmap key, which gives
//! weak-reference behavior for free.
//!
//! ## Lifecycle
//!
//! A node is **Unattached** when first created ([`new_leaf`], `parent ==
//! None`), **Attached** once [`add_child`] sets its parent, and **Freed**
//! when [`remove`] deletes it. `remove` cascades: it frees the whole
//! structural subtree (including internal helper nodes like the
//! scroll-view documentView wrapper). Freeing is always explicit — there
//! is no automatic refcount sweep — so a node that nothing has attached
//! persists until something frees it (this is what lets off-tree owners,
//! e.g. toolbar items, hold a node without a refcount).
//!
//! ## Why a custom tree
//!
//! Taffy's public `compute_layout_with_measure` hardcodes
//! `first_baselines: Point::NONE` for leaves, which breaks
//! `align_items: Baseline` for text. Implementing [`taffy::LayoutPartialTree`]
//! directly on [`LayoutState<B>`] lets each port populate
//! `LayoutOutput::first_baselines` from real font metrics.
//!
//! Layout *driving* (when to call `run_layout_pass`, how to dispatch to
//! the main thread) is left to each port.

use slotmap::{DefaultKey, SlotMap};
use std::cell::Cell;

use super::node::Node;

pub use taffy::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, JustifyContent, JustifyItems, Layout,
    LengthPercentage, LengthPercentageAuto, MaxTrackSizingFunction,
    MinTrackSizingFunction, NodeId, Point, Position, Rect, Size, Style,
    TrackSizingFunction,
};

/// Pre-monomorphized aliases for the grid types that carry a
/// `CheapCloneStr` generic. Taffy defaults this to `String` in std
/// builds, but the default parameter on `GridTemplateComponent` was
/// dropped in 0.10, so callers have to spell it.
pub type GridTemplateComponent = taffy::GridTemplateComponent<String>;
pub type GridTemplateRepetition = taffy::GridTemplateRepetition<String>;
pub type GridPlacement = taffy::GridPlacement<String>;
/// Convenience re-exports of Taffy's grid track-sizing constructors.
pub use taffy::style_helpers::{
    auto, fit_content, flex, fr, length, max_content, min_content, minmax,
    percent, repeat,
};
use taffy::{
    compute_cached_layout, compute_flexbox_layout, compute_grid_layout,
    compute_hidden_layout, compute_leaf_layout, compute_root_layout, round_layout,
    Cache, CacheTree, Display as TaffyDisplay, LayoutFlexboxContainer,
    LayoutGridContainer, LayoutInput, LayoutOutput, LayoutPartialTree, RoundTree,
    TraversePartialTree, TraverseTree,
};

// ---------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------

/// Platform glue between the layout engine and the native UI toolkit.
/// Each port (cocoa, iOS, GTK) provides one impl, including the
/// thread-local store accessor [`Self::with_tree`].
///
/// `View` is the cheap-to-clone reference type each port uses
/// (`Retained<NSView>`, `Retained<UIView>`, `gtk::Widget`).
///
/// `NodeMeta` is per-node side-data the port needs for layout that
/// doesn't fit in [`Style`] (e.g. cocoa's scroll-view flags). Set to
/// `()` if unused.
///
/// `Handlers` owns retained ObjC delegates / target objects whose
/// lifetime must track the node. Set to `()` for ports with none.
///
/// **Drop ordering**: when the store drops a node, the `handlers` field
/// drops BEFORE the `view` field so port-specific `Handlers` `Drop` impls
/// can nil `setTarget`/`setDelegate` on the still-live view.
///
/// Result of [`Backend::attach_native`]: how core should mirror a
/// just-performed native child-attach into the Taffy tree.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttachOutcome {
    /// Native insert done — mirror `child` under `parent` before the marker.
    Mirror,
    /// Native insert done, but `parent` isn't a Taffy container (e.g. a
    /// GTK window's single child / a content root) — do **not** mirror.
    NativeOnly,
    /// Nothing was inserted (self-parent, marker isn't `parent`'s child,
    /// unsupported container). Core reports the insert as failed.
    Rejected,
}

/// `Send + Sync` is required of the backend *marker type* (not its views —
/// those live pinned in the thread-local store): view-state structs embed
/// `PhantomData<B>` and must stay `Send` for leptos's `IntoView` bounds.
/// Real backends are fieldless unit structs, so this is free.
pub trait Backend: Send + Sync + 'static + Sized {
    /// The platform view associated with each node (cheap to clone).
    type View: Clone;
    /// Per-node port-specific metadata. `()` if unused.
    type NodeMeta: Clone + Default;
    /// Per-node port-specific handler storage. `()` if unused.
    type Handlers: Default + 'static;
    /// The port's color type (`leptos_cocoa::Color`, `gtk_dom::Color`, …).
    /// Used by the decoration hooks below and `DecorationAttrs`.
    type Color: 'static;

    /// Measure the intrinsic size of a leaf node's content. `known`
    /// carries any axis pinned by styles; return real values for the
    /// unknown axes. `available` is the parent's reported available
    /// space.
    fn measure_leaf(
        view: &Self::View,
        meta: &Self::NodeMeta,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
    ) -> Size<f32>;

    /// First-baseline offset from the top of `view`'s frame, or `None`
    /// for views with no measurable text baseline.
    fn first_baseline(view: &Self::View) -> Option<f32>;

    /// Run `f` with exclusive access to this backend's thread-local node
    /// store. The store is a main-thread-lifetime singleton; ports back
    /// it with a `thread_local!`.
    fn with_tree<R>(f: impl FnOnce(&mut LayoutState<Self>) -> R) -> R;

    // ---------------------------------------------------------------------
    // Native view setters — the platform-specific view mutations that the
    // core install loops (`apply_layout` / `apply_universal` /
    // `apply_decoration` in `setters.rs`) forward to. Same shape as
    // `measure_leaf`: a static fn over `&Self::View`.
    //
    // `set_hidden` / `set_alpha` are required (every UI toolkit has them);
    // `set_clip` / `set_tool_tip` default to no-op so a port without a clip
    // primitive or a touch port without tooltips degrades to layout-only /
    // silent, matching the prior driver-trait defaults.

    /// Toggle the view's visibility (`set_visible(!hidden)` / `isHidden`).
    fn set_hidden(view: &Self::View, hidden: bool);

    /// Toggle paint-time clipping of overflowing children
    /// (`Overflow::Hidden` / `masksToBounds` / `clipsToBounds`). No-op
    /// default: `overflow=Hidden` is layout-only on ports without it.
    fn set_clip(_view: &Self::View, _clip: bool) {}

    /// Set the view's opacity (0.0..=1.0).
    fn set_alpha(view: &Self::View, alpha: f64);

    /// Set the view's tooltip; empty string clears. No-op default for
    /// ports without a tooltip concept (e.g. touch).
    fn set_tool_tip(_view: &Self::View, _tip: &str) {}

    // Decoration hooks (layer-backed chrome). No-op defaults: a port
    // that doesn't support decoration simply doesn't expose the
    // `WithDecoration` builder methods (or warns, like GTK), so these
    // are unreachable there rather than silently ignored.

    /// Fill the view's background with `color`.
    fn set_background_color(_view: &Self::View, _color: Self::Color) {}

    /// Round the view's corners by `radius` points; `0.0` disables.
    fn set_corner_radius(_view: &Self::View, _radius: f32) {}

    /// Border stroke width in points; `0.0` disables.
    fn set_border_width(_view: &Self::View, _width: f32) {}

    /// Border stroke color (visible when `border_width > 0`).
    fn set_border_color(_view: &Self::View, _color: Self::Color) {}

    /// Drop-shadow color (visible when `shadow_opacity > 0`).
    fn set_shadow_color(_view: &Self::View, _color: Self::Color) {}

    /// Drop-shadow opacity, 0.0..=1.0; `0.0` disables.
    fn set_shadow_opacity(_view: &Self::View, _opacity: f32) {}

    /// Drop-shadow blur radius in points.
    fn set_shadow_radius(_view: &Self::View, _radius: f32) {}

    /// Drop-shadow offset as `(dx, dy)` points.
    fn set_shadow_offset(_view: &Self::View, _offset: (f32, f32)) {}

    /// Trigger a native re-measure/relayout for the subtree containing
    /// `id` (gtk `queue_resize`, cocoa/iOS main-queue dispatch). Required:
    /// scheduling is genuinely per-port, so there is no sensible default.
    fn schedule_relayout(id: NodeId);

    // ---------------------------------------------------------------------
    // Native tree edits. The port performs the platform-specific child
    // wiring (NSView `addSubview:` / gtk `insert_before` / UIKit
    // `insertSubview:`, including any redirects like a scroll view's
    // documentView); core ([`Node::insert_node`] etc.) drives them and
    // mirrors the edge into the Taffy tree. Taking `NodeId`s (not views)
    // lets the port read its own meta/redirects. These do NOT touch Taffy.
    // ---------------------------------------------------------------------

    /// Natively attach `child` under `parent`, before `before` (append if
    /// `None`). The [`AttachOutcome`] tells core whether to mirror the
    /// edge into Taffy.
    fn attach_native(parent: NodeId, child: NodeId, before: Option<NodeId>) -> AttachOutcome;

    /// Natively detach `child` from `parent`. Returns `false` if `child`
    /// wasn't actually a child of `parent` (core then skips the Taffy
    /// detach and reports "not removed").
    fn detach_native(parent: NodeId, child: NodeId) -> bool;

    /// Natively remove every child of `parent`. (Taffy is left to the
    /// caller / the remove cascade, matching prior per-port behavior.)
    fn clear_native_children(parent: NodeId);

    /// Natively detach `view` from whatever native parent currently
    /// holds it (`removeFromSuperview` / `gtk_widget_unparent`-flavoured).
    /// Called by [`Node::remove`] before the store entry is freed.
    fn remove_from_native_parent(view: &Self::View);

    // ---------------------------------------------------------------------
    // Node construction & text. The view-tree core (`Render` impls for
    // strings, control-flow markers, …) needs three per-port operations:
    // make a text node, make an invisible placeholder, update a text
    // node's content.
    // ---------------------------------------------------------------------

    /// Create a text node (a label-flavoured native view) showing `text`.
    fn create_text_node(text: &str) -> Node<Self>;

    /// Create an invisible placeholder node — the mount anchor used by
    /// control-flow constructs.
    fn create_placeholder() -> Node<Self>;

    /// Set the text content of a node created by
    /// [`Self::create_text_node`].
    fn set_text(node: Node<Self>, text: &str);

    /// Mounts `new_child` into the parent of `before`, immediately
    /// before `before`. Returns `false` if `before` has no parent (the
    /// caller then finds a different mount point).
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: Node<Self>) -> bool
    where
        M: crate::renderer::view::Mountable<Self>,
    {
        if let Some(parent) = before.parent() {
            new_child.mount(parent, Some(before));
            true
        } else {
            false
        }
    }

    // *** Utility methods. These methods are implemented in the trait to make them available for
    // downstream consumers. It is not expected that implementers of Backend override these
    // methods.

    fn new_leaf(
        style: Style,
        view: Self::View,
        meta: Self::NodeMeta,
        handlers: Self::Handlers,
    ) -> NodeId {
        Self::with_tree(|s| s.new_leaf(style, view, meta, handlers))
    }

    // ---------------------------------------------------------------------
    // Generic free-function API — the surface ports & node accessors call.
    // Each wraps `B::with_tree`. Names mirror the inherent methods.
    // ---------------------------------------------------------------------

    /// Remove a node and its structural subtree. Removed entries are dropped
    /// AFTER the store borrow releases, so port `Handlers`/view `Drop` impls
    /// can safely re-enter the store. Idempotent.
    fn remove(id: NodeId) {
        let mut removed = Vec::new();
        Self::with_tree(|s| s.remove_collect(id, &mut removed));
        drop(removed);
    }

    fn add_child(parent: NodeId, child: NodeId) {
        Self::with_tree(|s| s.add_child(parent, child))
    }

    fn insert_child_at_index(parent: NodeId, idx: usize, child: NodeId) {
        Self::with_tree(|s| s.insert_child_at_index(parent, idx, child))
    }

    fn insert_child_before(parent: NodeId, child: NodeId, marker: Option<NodeId>) {
        Self::with_tree(|s| s.insert_child_before(parent, child, marker))
    }

    fn remove_child(parent: NodeId, child: NodeId) {
        Self::with_tree(|s| s.remove_child(parent, child))
    }

    fn mark_dirty(id: NodeId) {
        Self::with_tree(|s| s.mark_dirty(id))
    }

    fn set_style(id: NodeId, style: Style) {
        Self::with_tree(|s| s.set_style(id, style))
    }

    fn style(id: NodeId) -> Option<Style> {
        Self::with_tree(|s| s.style(id))
    }

    fn set_meta(id: NodeId, meta: Self::NodeMeta) {
        Self::with_tree(|s| s.set_meta(id, meta))
    }

    fn meta(id: NodeId) -> Option<Self::NodeMeta> {
        Self::with_tree(|s| s.meta(id))
    }

    fn set_debug_tag_name(id: NodeId, name: &'static str) {
        Self::with_tree(|s| s.set_debug_tag_name(id, name))
    }

    fn debug_tag_name(id: NodeId) -> &'static str {
        Self::with_tree(|s| s.debug_tag_name(id))
    }

    /// Every live node id (unspecified order). For debug tooling.
    fn all_node_ids() -> Vec<NodeId> {
        Self::with_tree(|s| s.all_node_ids())
    }

    /// Subtree roots — live nodes with no parent. For debug tooling.
    fn roots() -> Vec<NodeId> {
        Self::with_tree(|s| s.roots())
    }

    fn view(id: NodeId) -> Option<Self::View> {
        Self::with_tree(|s| s.view(id))
    }

    fn parent(id: NodeId) -> Option<NodeId> {
        Self::with_tree(|s| s.parent(id))
    }

    fn children(id: NodeId) -> Vec<NodeId> {
        Self::with_tree(|s| s.children(id))
    }

    fn layout(id: NodeId) -> Option<Layout> {
        Self::with_tree(|s| s.layout(id))
    }

    fn set_final_layout(id: NodeId, l: Layout) {
        Self::with_tree(|s| s.set_final_layout(id, l))
    }

    fn dirty(id: NodeId) -> bool {
        Self::with_tree(|s| s.dirty(id))
    }

    fn get_node_context(id: NodeId) -> Option<NodeContext<Self>> {
        Self::with_tree(|s| s.get_node_context(id))
    }

    fn node_count() -> usize {
        Self::with_tree(|s| s.node_count())
    }

    /// `true` if `id` is currently present in the store.
    fn contains(id: NodeId) -> bool {
        Self::with_tree(|s| s.nodes.contains_key(key(id)))
    }

    fn with_handlers_mut<R>(
        id: NodeId,
        f: impl FnOnce(&mut Self::Handlers) -> R,
    ) -> Option<R> {
        Self::with_tree(|s| s.with_handlers_mut(id, f))
    }

    /// The topmost ancestor of `id` (its subtree root) — walk `parent`
    /// until there is none. Safe against freed intermediates: a missing
    /// node has no parent, so the walk stops. Returns `id` itself if it's
    /// already a root or absent.
    fn root_of(id: NodeId) -> NodeId {
        Self::with_tree(|s| {
            let mut cur = id;
            while let Some(p) = s.parent(cur) {
                cur = p;
            }
            cur
        })
    }

    /// Enqueue `root` for recompute on the next relayout pass (deduped).
    fn queue_relayout(root: NodeId) {
        Self::with_tree(|s| {
            if !s.pending_relayout.contains(&root) {
                s.pending_relayout.push(root);
            }
        })
    }

    /// Drain the set of roots queued for recompute.
    fn take_pending_relayout() -> Vec<NodeId> {
        Self::with_tree(|s| std::mem::take(&mut s.pending_relayout))
    }

    fn relayout_queued() -> bool {
        Self::with_tree(|s| s.relayout_queued.get())
    }

    fn set_relayout_queued(v: bool) {
        Self::with_tree(|s| s.relayout_queued.set(v))
    }

    fn run_layout_pass(id: NodeId, available: Size<AvailableSpace>) {
        Self::with_tree(|s| s.run_layout_pass(id, available))
    }

    /// Pre-order `(id, layout, view)` snapshot for the subtree rooted at
    /// `id`; safe to apply frames after this returns (the store borrow is
    /// already released).
    fn collect_subtree(id: NodeId) -> Vec<(NodeId, Layout, Self::View)> {
        Self::with_tree(|s| s.collect_subtree(id))
    }

}

// ---------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------

/// Lightweight read-only snapshot of a node's view + backend metadata.
#[derive(Clone)]
pub struct NodeContext<B: Backend> {
    pub view: B::View,
    pub meta: B::NodeMeta,
}

/// The per-thread node store: a generational slotmap of [`NodeData`]
/// plus the relayout work-queue. Ports hold one of these in a
/// `thread_local!` and expose it via [`Backend::with_tree`].
pub struct LayoutState<B: Backend> {
    nodes: SlotMap<DefaultKey, NodeData<B>>,
    /// Subtree roots queued for recompute on the next main-loop tick.
    /// The port's relayout scheduler walks up from a mutated node to
    /// its root and enqueues just that root, so a change in one window
    /// doesn't recompute the others. Drained when the pass runs; a root
    /// freed before then resolves to `None` on lookup and is skipped
    /// (generational keys never alias).
    pending_relayout: Vec<NodeId>,
    /// Dedup flag for the port's relayout scheduler: `true` while a
    /// relayout pass is queued for the next main-loop tick.
    pub relayout_queued: Cell<bool>,
}

impl<B: Backend> Default for LayoutState<B> {
    fn default() -> Self {
        LayoutState {
            nodes: SlotMap::new(),
            pending_relayout: Vec::new(),
            relayout_queued: Cell::new(false),
        }
    }
}

struct NodeData<B: Backend> {
    style: Style,
    cache: Cache,
    unrounded_layout: Layout,
    final_layout: Layout,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Human-readable element kind for debug tooling (devtools, overlay):
    /// `"button"`, `"vstack"`, … Set once at construction by the port's
    /// `make_view`; `""` if unset. Never shown to end users.
    debug_tag_name: &'static str,
    // IMPORTANT: `handlers` MUST come before `view` so it drops first.
    // Port `Handlers` Drop impls nil setTarget/setDelegate on the
    // still-live view to sever AppKit/UIKit dispatch into freed memory.
    handlers: B::Handlers,
    view: B::View,
    meta: B::NodeMeta,
}

impl<B: Backend> NodeData<B> {
    fn new(style: Style, view: B::View, meta: B::NodeMeta, handlers: B::Handlers) -> Self {
        NodeData {
            style,
            cache: Cache::new(),
            unrounded_layout: Layout::new(),
            final_layout: Layout::new(),
            parent: None,
            children: Vec::new(),
            debug_tag_name: "",
            handlers,
            view,
            meta,
        }
    }
}

fn key(id: NodeId) -> DefaultKey {
    id.into()
}

// ---------------------------------------------------------------------
// LayoutState inherent API (operates on the borrowed store)
// ---------------------------------------------------------------------

impl<B: Backend> LayoutState<B> {
    fn with_node<R>(&self, id: NodeId, f: impl FnOnce(&NodeData<B>) -> R) -> Option<R> {
        self.nodes.get(key(id)).map(f)
    }

    /// Run a Taffy layout pass rooted at `id`.
    pub fn run_layout_pass(&mut self, id: NodeId, available_space: Size<AvailableSpace>) {
        compute_root_layout(self, id, available_space);
        round_layout(self, id);
    }

    /// Total number of nodes (including unattached orphans). Used by
    /// leak detectors to verify teardown returned the store to baseline.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Insert a fresh leaf, returning its `NodeId`. The node starts
    /// Unattached (`parent == None`); attach it with [`Self::add_child`]
    /// or it persists until explicitly [`Self::remove`]d.
    pub fn new_leaf(
        &mut self,
        style: Style,
        view: B::View,
        meta: B::NodeMeta,
        handlers: B::Handlers,
    ) -> NodeId {
        NodeId::from(self.nodes.insert(NodeData::new(style, view, meta, handlers)))
    }

    /// Run a closure with exclusive access to a node's handler storage.
    /// `None` if the node isn't in the store.
    pub fn with_handlers_mut<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut B::Handlers) -> R,
    ) -> Option<R> {
        self.nodes.get_mut(key(id)).map(|n| f(&mut n.handlers))
    }

    /// Remove a node and its whole structural subtree. Returns the
    /// removed [`NodeData`] entries so the caller can drop them OUTSIDE
    /// the store borrow (their `Drop` may otherwise re-enter). Idempotent:
    /// a stale id contributes nothing.
    fn remove_collect(&mut self, id: NodeId, out: &mut Vec<NodeData<B>>) {
        let Some((parent, kids)) = self
            .nodes
            .get_mut(key(id))
            .map(|n| (n.parent, std::mem::take(&mut n.children)))
        else {
            return;
        };
        // Detach from parent's child list.
        if let Some(p) = parent {
            if let Some(p_data) = self.nodes.get_mut(key(p)) {
                p_data.children.retain(|c| *c != id);
            }
        }
        // Cascade into the whole subtree.
        for c in kids {
            self.remove_collect(c, out);
        }
        if let Some(removed) = self.nodes.remove(key(id)) {
            out.push(removed);
        }
        // Invalidate the (former) parent's cached layout.
        if let Some(p) = parent {
            self.mark_dirty(p);
        }
    }

    /// Add a child edge. Idempotent. Detaches `child` from any previous
    /// parent first (a node has one parent, like `addSubview:`).
    pub fn add_child(&mut self, parent: NodeId, child: NodeId) {
        let prev_parent = self.nodes.get(key(child)).and_then(|c| c.parent);
        if prev_parent == Some(parent) {
            let already = self
                .nodes
                .get(key(parent))
                .map(|p| p.children.contains(&child))
                .unwrap_or(false);
            if already {
                return;
            }
        } else if let Some(prev) = prev_parent {
            if let Some(p_data) = self.nodes.get_mut(key(prev)) {
                p_data.children.retain(|c| *c != child);
            }
        }
        if let Some(p) = self.nodes.get_mut(key(parent)) {
            if !p.children.contains(&child) {
                p.children.push(child);
            }
        }
        if let Some(c) = self.nodes.get_mut(key(child)) {
            c.parent = Some(parent);
        }
        self.mark_dirty(parent);
        if let Some(prev) = prev_parent {
            if prev != parent {
                self.mark_dirty(prev);
            }
        }
    }

    /// Place `child` at exactly `idx` under `parent`, moving/detaching
    /// as needed.
    pub fn insert_child_at_index(&mut self, parent: NodeId, idx: usize, child: NodeId) {
        let prev = self.nodes.get(key(child)).and_then(|c| c.parent);
        if prev != Some(parent) {
            if let Some(prev) = prev {
                if let Some(p_data) = self.nodes.get_mut(key(prev)) {
                    p_data.children.retain(|c| *c != child);
                }
            }
        }
        if let Some(p) = self.nodes.get_mut(key(parent)) {
            p.children.retain(|c| *c != child);
            let i = idx.min(p.children.len());
            p.children.insert(i, child);
        }
        if let Some(c) = self.nodes.get_mut(key(child)) {
            c.parent = Some(parent);
        }
        self.mark_dirty(parent);
        if let Some(prev) = prev {
            if prev != parent {
                self.mark_dirty(prev);
            }
        }
    }

    /// Insert `child` under `parent` immediately before `marker`. If
    /// `marker` is `None`, or isn't currently a child of `parent`, append.
    /// Detaches `child` from any previous parent first (one-parent rule,
    /// like [`Self::add_child`]). The marker index is computed against the
    /// child list with `child` itself excluded, so reordering a node that's
    /// already a child of `parent` lands at the right slot.
    ///
    /// This is the marker-based counterpart to [`Self::insert_child_at_index`]:
    /// it lets a port mirror a native insert-before-sibling into Taffy
    /// **without** reading the realized native child index back. (See the
    /// insertion canary: whether ports can rely on this instead of the
    /// native-order readback is exactly what's under test.)
    pub fn insert_child_before(&mut self, parent: NodeId, child: NodeId, marker: Option<NodeId>) {
        // Insert-before-self is a no-op (a node is already "before" itself);
        // without this guard the `others()` filter drops `child`, fails to
        // find the marker, and appends — silently reordering.
        if marker == Some(child) {
            return;
        }
        let idx = self
            .nodes
            .get(key(parent))
            .map(|p| {
                let others = || p.children.iter().filter(|c| **c != child);
                match marker {
                    Some(m) => others()
                        .position(|c| *c == m)
                        .unwrap_or_else(|| others().count()),
                    None => others().count(),
                }
            })
            .unwrap_or(0);
        self.insert_child_at_index(parent, idx, child);
    }

    /// Detach `child` from `parent` (does NOT free it). The node becomes
    /// Unattached and persists until explicitly removed.
    pub fn remove_child(&mut self, parent: NodeId, child: NodeId) {
        if let Some(p) = self.nodes.get_mut(key(parent)) {
            p.children.retain(|c| *c != child);
        }
        if let Some(c) = self.nodes.get_mut(key(child)) {
            if c.parent == Some(parent) {
                c.parent = None;
            }
        }
        self.mark_dirty(parent);
    }

    /// Mark `id` and its ancestors dirty (cleared cache → forced
    /// re-layout). Stops as soon as it hits an already-dirty node.
    pub fn mark_dirty(&mut self, id: NodeId) {
        let mut current = Some(id);
        while let Some(c) = current {
            let Some(node) = self.nodes.get_mut(key(c)) else { break };
            if node.cache.is_empty() {
                break;
            }
            node.cache.clear();
            current = node.parent;
        }
    }

    pub fn set_style(&mut self, id: NodeId, style: Style) {
        if let Some(node) = self.nodes.get_mut(key(id)) {
            node.style = style;
        }
        self.mark_dirty(id);
    }

    /// Mutate a node's style in place and mark it dirty — the zero-clone
    /// fast path behind [`Node::with_style_mut`]. Taffy's `Style` carries
    /// grid-template `Vec`s, so the old clone → mutate → write-back cycle
    /// allocated on every setter call. A missing node mutates a scratch
    /// default (no-op).
    pub fn update_style<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut Style) -> R,
    ) -> R {
        match self.nodes.get_mut(key(id)) {
            Some(node) => {
                let r = f(&mut node.style);
                self.mark_dirty(id);
                r
            }
            None => f(&mut Style::default()),
        }
    }

    /// Mutate a node's backend metadata in place. A missing node
    /// mutates a scratch default (no-op).
    pub fn update_meta<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut B::NodeMeta) -> R,
    ) -> R {
        match self.nodes.get_mut(key(id)) {
            Some(node) => f(&mut node.meta),
            None => f(&mut B::NodeMeta::default()),
        }
    }

    /// Override the final (rounded) layout for a node. Used by ports
    /// that run a second compute pass on a subtree.
    pub fn set_final_layout(&mut self, id: NodeId, layout: Layout) {
        if let Some(node) = self.nodes.get_mut(key(id)) {
            node.final_layout = layout;
        }
    }

    /// Update a node's port-specific meta.
    pub fn set_meta(&mut self, id: NodeId, meta: B::NodeMeta) {
        if let Some(node) = self.nodes.get_mut(key(id)) {
            node.meta = meta;
        }
    }

    /// Record the element kind for debug tooling. Set once, just after
    /// the node is created.
    pub fn set_debug_tag_name(&mut self, id: NodeId, name: &'static str) {
        if let Some(node) = self.nodes.get_mut(key(id)) {
            node.debug_tag_name = name;
        }
    }

    // -- read accessors -----------------------------------------------

    /// Element kind recorded by the port (`""` if unset).
    pub fn debug_tag_name(&self, id: NodeId) -> &'static str {
        self.with_node(id, |n| n.debug_tag_name).unwrap_or("")
    }

    /// Every live node id, in unspecified order. For debug tree-walking.
    pub fn all_node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().map(NodeId::from).collect()
    }

    /// Live nodes with no parent — the subtree roots (one per window /
    /// scene, plus any detached/unattached nodes). For debug tree-walking.
    pub fn roots(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.parent.is_none())
            .map(|(k, _)| NodeId::from(k))
            .collect()
    }

    /// Final (rounded) layout.
    pub fn layout(&self, id: NodeId) -> Option<Layout> {
        self.with_node(id, |n| n.final_layout)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.with_node(id, |n| n.parent).flatten()
    }

    /// Children of `id` (cloned). Empty if the node isn't in the store.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.with_node(id, |n| n.children.clone()).unwrap_or_default()
    }

    pub fn dirty(&self, id: NodeId) -> bool {
        self.with_node(id, |n| n.cache.is_empty()).unwrap_or(true)
    }

    pub fn get_node_context(&self, id: NodeId) -> Option<NodeContext<B>> {
        self.with_node(id, |n| NodeContext {
            view: n.view.clone(),
            meta: n.meta.clone(),
        })
    }

    pub fn style(&self, id: NodeId) -> Option<Style> {
        self.with_node(id, |n| n.style.clone())
    }

    /// Borrowed view of a node's style (no clone).
    pub fn style_ref(&self, id: NodeId) -> Option<&Style> {
        self.nodes.get(key(id)).map(|n| &n.style)
    }

    /// Borrowed view of a node's backend metadata (no clone).
    pub fn meta_ref(&self, id: NodeId) -> Option<&B::NodeMeta> {
        self.nodes.get(key(id)).map(|n| &n.meta)
    }

    /// Cheap accessor for the platform view.
    pub fn view(&self, id: NodeId) -> Option<B::View> {
        self.with_node(id, |n| n.view.clone())
    }

    /// Cheap accessor for the per-node backend metadata.
    pub fn meta(&self, id: NodeId) -> Option<B::NodeMeta> {
        self.with_node(id, |n| n.meta.clone())
    }

    /// Pre-order snapshot of `(id, layout, view)` for the subtree rooted
    /// at `id`, for nodes that have both a stored layout and a view.
    /// Returns a Vec so the caller can apply frames (which message the
    /// platform views) AFTER releasing the store borrow.
    pub fn collect_subtree(&self, id: NodeId) -> Vec<(NodeId, Layout, B::View)> {
        let mut out = Vec::new();
        self.collect_subtree_into(id, &mut out);
        out
    }

    fn collect_subtree_into(&self, id: NodeId, out: &mut Vec<(NodeId, Layout, B::View)>) {
        let Some(n) = self.nodes.get(key(id)) else { return };
        out.push((id, n.final_layout, n.view.clone()));
        for child in &n.children {
            self.collect_subtree_into(*child, out);
        }
    }
}

// ---------------------------------------------------------------------
// Taffy trait impls — operate on `LayoutState<B>`.
// ---------------------------------------------------------------------

impl<B: Backend> TraversePartialTree for LayoutState<B> {
    type ChildIter<'a>
        = std::iter::Copied<std::slice::Iter<'a, NodeId>>
    where
        Self: 'a;

    fn child_ids(&self, parent: NodeId) -> Self::ChildIter<'_> {
        self.nodes
            .get(key(parent))
            .map(|n| n.children.as_slice())
            .unwrap_or(&[])
            .iter()
            .copied()
    }

    fn child_count(&self, parent: NodeId) -> usize {
        self.nodes.get(key(parent)).map(|n| n.children.len()).unwrap_or(0)
    }

    fn get_child_id(&self, parent: NodeId, index: usize) -> NodeId {
        self.nodes[key(parent)].children[index]
    }
}

impl<B: Backend> TraverseTree for LayoutState<B> {}

impl<B: Backend> CacheTree for LayoutState<B> {
    fn cache_get(&self, id: NodeId, input: &LayoutInput) -> Option<LayoutOutput> {
        self.nodes[key(id)].cache.get(input)
    }

    fn cache_store(&mut self, id: NodeId, input: &LayoutInput, output: LayoutOutput) {
        self.nodes[key(id)].cache.store(input, output)
    }

    fn cache_clear(&mut self, id: NodeId) {
        self.nodes[key(id)].cache.clear();
    }
}

impl<B: Backend> LayoutPartialTree for LayoutState<B> {
    type CoreContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type CustomIdent = String;

    fn get_core_container_style(&self, id: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.nodes[key(id)].style
    }

    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        self.nodes[key(id)].unrounded_layout = *layout;
    }

    fn resolve_calc_value(&self, _: *const (), _: f32) -> f32 {
        0.0
    }

    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        // Hidden-layout passes recurse into visible descendants of a
        // `Display::None` node; taffy forbids running a leaf measure
        // function in that mode (unreachable!), so short-circuit the
        // whole subtree the way taffy's own TaffyTree does.
        if inputs.run_mode == taffy::RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |this, id, inputs| {
            let k = key(id);
            let display = this.nodes[k].style.display;
            let has_children = !this.nodes[k].children.is_empty();

            match (display, has_children) {
                (TaffyDisplay::None, _) => compute_hidden_layout(this, id),
                (TaffyDisplay::Flex, true) => compute_flexbox_layout(this, id, inputs),
                (TaffyDisplay::Grid, true) => compute_grid_layout(this, id, inputs),
                (_, false) => {
                    let node = &this.nodes[k];
                    let mut out = compute_leaf_layout(
                        inputs,
                        &node.style,
                        |_, _| 0.0,
                        |known, avail| B::measure_leaf(&node.view, &node.meta, known, avail),
                    );
                    out.first_baselines.y = B::first_baseline(&node.view);
                    out
                }
            }
        })
    }
}

impl<B: Backend> LayoutFlexboxContainer for LayoutState<B> {
    type FlexboxContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type FlexboxItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    fn get_flexbox_container_style(&self, id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        &self.nodes[key(id)].style
    }

    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.nodes[key(id)].style
    }
}

impl<B: Backend> LayoutGridContainer for LayoutState<B> {
    type GridContainerStyle<'a>
        = &'a Style
    where
        Self: 'a;
    type GridItemStyle<'a>
        = &'a Style
    where
        Self: 'a;

    fn get_grid_container_style(&self, id: NodeId) -> Self::GridContainerStyle<'_> {
        &self.nodes[key(id)].style
    }

    fn get_grid_child_style(&self, id: NodeId) -> Self::GridItemStyle<'_> {
        &self.nodes[key(id)].style
    }
}

impl<B: Backend> RoundTree for LayoutState<B> {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.nodes[key(id)].unrounded_layout
    }

    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.nodes[key(id)].final_layout = *layout;
    }
}
