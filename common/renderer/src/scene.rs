//! Renderer-agnostic layout engine over a **thread-local node store**.
//!
//! Every node lives in a single per-thread [`LayoutState<B>`] slotmap,
//! reached through [`LayoutBackend::with_tree`]. A node reference is a
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
pub trait LayoutBackend: 'static + Sized {
    /// The platform view associated with each node (cheap to clone).
    type View: Clone;
    /// Per-node port-specific metadata. `()` if unused.
    type NodeMeta: Clone + Default;
    /// Per-node port-specific handler storage. `()` if unused.
    type Handlers: Default + 'static;

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
}

// ---------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------

/// Lightweight read-only snapshot of a node's view + backend metadata.
#[derive(Clone)]
pub struct NodeContext<B: LayoutBackend> {
    pub view: B::View,
    pub meta: B::NodeMeta,
}

/// The per-thread node store: a generational slotmap of [`NodeData`]
/// plus the relayout work-queue. Ports hold one of these in a
/// `thread_local!` and expose it via [`LayoutBackend::with_tree`].
pub struct LayoutState<B: LayoutBackend> {
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

impl<B: LayoutBackend> Default for LayoutState<B> {
    fn default() -> Self {
        LayoutState {
            nodes: SlotMap::new(),
            pending_relayout: Vec::new(),
            relayout_queued: Cell::new(false),
        }
    }
}

struct NodeData<B: LayoutBackend> {
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

impl<B: LayoutBackend> NodeData<B> {
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

impl<B: LayoutBackend> LayoutState<B> {
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
            .get(key(id))
            .map(|n| (n.parent, n.children.clone()))
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
        if let (Some(layout), Some(view)) = (self.layout(id), self.view(id)) {
            out.push((id, layout, view));
        }
        if let Some(n) = self.nodes.get(key(id)) {
            for child in n.children.clone() {
                self.collect_subtree_into(child, out);
            }
        }
    }
}

// ---------------------------------------------------------------------
// Generic free-function API — the surface ports & node accessors call.
// Each wraps `B::with_tree`. Names mirror the inherent methods.
// ---------------------------------------------------------------------

pub fn new_leaf<B: LayoutBackend>(
    style: Style,
    view: B::View,
    meta: B::NodeMeta,
    handlers: B::Handlers,
) -> NodeId {
    B::with_tree(|s| s.new_leaf(style, view, meta, handlers))
}

/// Remove a node and its structural subtree. Removed entries are dropped
/// AFTER the store borrow releases, so port `Handlers`/view `Drop` impls
/// can safely re-enter the store. Idempotent.
pub fn remove<B: LayoutBackend>(id: NodeId) {
    let mut removed = Vec::new();
    B::with_tree(|s| s.remove_collect(id, &mut removed));
    drop(removed);
}

pub fn add_child<B: LayoutBackend>(parent: NodeId, child: NodeId) {
    B::with_tree(|s| s.add_child(parent, child))
}

pub fn insert_child_at_index<B: LayoutBackend>(parent: NodeId, idx: usize, child: NodeId) {
    B::with_tree(|s| s.insert_child_at_index(parent, idx, child))
}

pub fn remove_child<B: LayoutBackend>(parent: NodeId, child: NodeId) {
    B::with_tree(|s| s.remove_child(parent, child))
}

pub fn mark_dirty<B: LayoutBackend>(id: NodeId) {
    B::with_tree(|s| s.mark_dirty(id))
}

pub fn set_style<B: LayoutBackend>(id: NodeId, style: Style) {
    B::with_tree(|s| s.set_style(id, style))
}

pub fn style<B: LayoutBackend>(id: NodeId) -> Option<Style> {
    B::with_tree(|s| s.style(id))
}

pub fn set_meta<B: LayoutBackend>(id: NodeId, meta: B::NodeMeta) {
    B::with_tree(|s| s.set_meta(id, meta))
}

pub fn meta<B: LayoutBackend>(id: NodeId) -> Option<B::NodeMeta> {
    B::with_tree(|s| s.meta(id))
}

pub fn set_debug_tag_name<B: LayoutBackend>(id: NodeId, name: &'static str) {
    B::with_tree(|s| s.set_debug_tag_name(id, name))
}

pub fn debug_tag_name<B: LayoutBackend>(id: NodeId) -> &'static str {
    B::with_tree(|s| s.debug_tag_name(id))
}

/// Every live node id (unspecified order). For debug tooling.
pub fn all_node_ids<B: LayoutBackend>() -> Vec<NodeId> {
    B::with_tree(|s| s.all_node_ids())
}

/// Subtree roots — live nodes with no parent. For debug tooling.
pub fn roots<B: LayoutBackend>() -> Vec<NodeId> {
    B::with_tree(|s| s.roots())
}

pub fn view<B: LayoutBackend>(id: NodeId) -> Option<B::View> {
    B::with_tree(|s| s.view(id))
}

pub fn parent<B: LayoutBackend>(id: NodeId) -> Option<NodeId> {
    B::with_tree(|s| s.parent(id))
}

pub fn children<B: LayoutBackend>(id: NodeId) -> Vec<NodeId> {
    B::with_tree(|s| s.children(id))
}

pub fn layout<B: LayoutBackend>(id: NodeId) -> Option<Layout> {
    B::with_tree(|s| s.layout(id))
}

pub fn set_final_layout<B: LayoutBackend>(id: NodeId, l: Layout) {
    B::with_tree(|s| s.set_final_layout(id, l))
}

pub fn dirty<B: LayoutBackend>(id: NodeId) -> bool {
    B::with_tree(|s| s.dirty(id))
}

pub fn get_node_context<B: LayoutBackend>(id: NodeId) -> Option<NodeContext<B>> {
    B::with_tree(|s| s.get_node_context(id))
}

pub fn node_count<B: LayoutBackend>() -> usize {
    B::with_tree(|s| s.node_count())
}

/// `true` if `id` is currently present in the store.
pub fn contains<B: LayoutBackend>(id: NodeId) -> bool {
    B::with_tree(|s| s.nodes.contains_key(key(id)))
}

pub fn with_handlers_mut<B: LayoutBackend, R>(
    id: NodeId,
    f: impl FnOnce(&mut B::Handlers) -> R,
) -> Option<R> {
    B::with_tree(|s| s.with_handlers_mut(id, f))
}

/// The topmost ancestor of `id` (its subtree root) — walk `parent`
/// until there is none. Safe against freed intermediates: a missing
/// node has no parent, so the walk stops. Returns `id` itself if it's
/// already a root or absent.
pub fn root_of<B: LayoutBackend>(id: NodeId) -> NodeId {
    B::with_tree(|s| {
        let mut cur = id;
        while let Some(p) = s.parent(cur) {
            cur = p;
        }
        cur
    })
}

/// Enqueue `root` for recompute on the next relayout pass (deduped).
pub fn queue_relayout<B: LayoutBackend>(root: NodeId) {
    B::with_tree(|s| {
        if !s.pending_relayout.contains(&root) {
            s.pending_relayout.push(root);
        }
    })
}

/// Drain the set of roots queued for recompute.
pub fn take_pending_relayout<B: LayoutBackend>() -> Vec<NodeId> {
    B::with_tree(|s| std::mem::take(&mut s.pending_relayout))
}

pub fn relayout_queued<B: LayoutBackend>() -> bool {
    B::with_tree(|s| s.relayout_queued.get())
}

pub fn set_relayout_queued<B: LayoutBackend>(v: bool) {
    B::with_tree(|s| s.relayout_queued.set(v))
}

pub fn run_layout_pass<B: LayoutBackend>(id: NodeId, available: Size<AvailableSpace>) {
    B::with_tree(|s| s.run_layout_pass(id, available))
}

/// Pre-order `(id, layout, view)` snapshot for the subtree rooted at
/// `id`; safe to apply frames after this returns (the store borrow is
/// already released).
pub fn collect_subtree<B: LayoutBackend>(id: NodeId) -> Vec<(NodeId, Layout, B::View)> {
    B::with_tree(|s| s.collect_subtree(id))
}

// ---------------------------------------------------------------------
// Taffy trait impls — operate on `LayoutState<B>`.
// ---------------------------------------------------------------------

impl<B: LayoutBackend> TraversePartialTree for LayoutState<B> {
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

impl<B: LayoutBackend> TraverseTree for LayoutState<B> {}

impl<B: LayoutBackend> CacheTree for LayoutState<B> {
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

impl<B: LayoutBackend> LayoutPartialTree for LayoutState<B> {
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

impl<B: LayoutBackend> LayoutFlexboxContainer for LayoutState<B> {
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

impl<B: LayoutBackend> LayoutGridContainer for LayoutState<B> {
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

impl<B: LayoutBackend> RoundTree for LayoutState<B> {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.nodes[key(id)].unrounded_layout
    }

    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.nodes[key(id)].final_layout = *layout;
    }
}
