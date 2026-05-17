//! Renderer-agnostic layout engine.
//!
//! Provides a Taffy-backed [`LayoutTree<B>`] generic over a
//! [`LayoutBackend`]. Each native port (cocoa, iOS, GTK) implements
//! the trait by supplying its platform view type plus three
//! operations: measure a leaf, query its first text baseline, and
//! apply a computed frame.
//!
//! ## Why a custom tree
//!
//! Taffy's public `compute_layout_with_measure` API hardcodes
//! `first_baselines: Point::NONE` for leaves regardless of what the
//! measure callback returns, which breaks `align_items: Baseline`
//! for any text-bearing leaf. Implementing
//! [`taffy::LayoutPartialTree`] directly lets each port populate
//! `LayoutOutput::first_baselines` from its real font metrics, so
//! Taffy's flexbox engine performs baseline alignment properly.
//!
//! ## What's in here
//!
//! - [`LayoutTree<B>`] — owns the per-node data
//!   (style/cache/layout/parent/children/view/meta) for one window
//!   or scene.
//! - [`LayoutBackend`] — the trait each port implements.
//! - [`NodeLayout<B>`] — the per-node "layout slot" each port's
//!   element wrapper embeds. Holds the node's style and an
//!   `Option<LayoutHandle<B>>` (set once the node has been
//!   registered into a tree).
//! - [`LayoutHandle<B>`] — `(TreeRef<B>, NodeId)` pair the port
//!   carries on each registered element.
//! - [`NodeContext<B>`] — read-only snapshot of a node's view +
//!   backend metadata, exposed for things like the cocoa debug
//!   overlay.
//!
//! Layout *driving* (when to call `compute_layout`, how to dispatch
//! to the main thread, etc.) is left to each port — it's tied up
//! in main-loop / dispatch concerns that don't generalise.

use slotmap::{DefaultKey, SlotMap};
use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

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
/// dropped in 0.10, so callers have to spell it. These aliases keep
/// downstream code free of the generic noise — match Taffy's
/// `Style<String>` default.
pub type GridTemplateComponent = taffy::GridTemplateComponent<String>;
pub type GridTemplateRepetition = taffy::GridTemplateRepetition<String>;
pub type GridPlacement = taffy::GridPlacement<String>;
/// Convenience re-exports of Taffy's grid track-sizing constructors —
/// `fr(2.0)`, `length(120.0)`, `percent(0.5)`, `auto()`, `min_content()`,
/// `max_content()`, `minmax(min, max)`, `fit_content(...)`, `repeat(n, …)`,
/// `flex(n)`. Caller code can build a `Vec<GridTemplateComponent>`
/// without an explicit `taffy::` import.
///
/// Note: Taffy also has `line(i16)` / `span(u16)` for building
/// `GridPlacement` values directly, but the high-level builder API
/// uses [`renderer::attrs::GridLine`] for per-item placement and
/// re-exports its own `span` from there — so we deliberately don't
/// re-export Taffy's here, to keep prelude imports unambiguous.
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

/// Platform glue between the layout engine and the native UI
/// toolkit. Each port (cocoa, iOS, GTK) provides one impl.
///
/// `View` is the cheap-to-clone reference type each port uses
/// (`Retained<NSView>`, `Retained<UIView>`, `gtk::Widget`). It's
/// stored once per node and queried by `measure_leaf`,
/// `first_baseline`, and `set_frame`.
///
/// `NodeMeta` is per-node side-data the port needs for layout that
/// doesn't fit in [`Style`]. Today this is used by cocoa/iOS to flag
/// scroll-view nodes (so the port can run a second compute pass over
/// scroll-view subtrees with a different available-space). Set to
/// `()` for ports that don't need it.
pub trait LayoutBackend: 'static + Sized {
    /// The platform view associated with each node. Cheap to clone
    /// (typically a retained pointer or smart pointer to a widget).
    type View: Clone;

    /// Per-node port-specific metadata. Set to `()` if unused.
    type NodeMeta: Clone + Default;

    /// Measure the intrinsic size of a leaf node's content.
    ///
    /// `known` carries any axis already pinned by styles
    /// (`size: length(...)`); the impl should return real values for
    /// the unknown axes. Returning the known values too is fine —
    /// the engine clamps them anyway.
    ///
    /// `available` is the parent's reported available space — useful
    /// for views whose size depends on it (e.g. wrapping text fields
    /// laying out for the width Taffy will give them).
    fn measure_leaf(
        view: &Self::View,
        meta: &Self::NodeMeta,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
    ) -> Size<f32>;

    /// First-baseline offset from the top of `view`'s frame.
    ///
    /// Returns `None` for views with no measurable text baseline
    /// (containers, image views, etc.). The returned value is
    /// frame-relative (not alignment-rect-relative); convert at the
    /// call site if your platform reports baselines differently.
    fn first_baseline(view: &Self::View) -> Option<f32>;
}

// ---------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------

/// Lightweight read-only snapshot of a node's per-tree context.
/// Returned by [`LayoutTree::get_node_context`] for callers that need
/// to inspect a node without borrowing the whole tree (e.g. the cocoa
/// debug overlay).
#[derive(Clone)]
pub struct NodeContext<B: LayoutBackend> {
    pub view: B::View,
    pub meta: B::NodeMeta,
}

/// Owns the layout tree plus a slot for the tree's root NodeId. One
/// per window/scene; each registered element carries an Rc clone via
/// its [`LayoutHandle`].
pub struct LayoutTree<B: LayoutBackend> {
    state: RefCell<LayoutState<B>>,
    /// Set by the port's `register_in_tree` helper the first time a
    /// tree gets a node. Tracked explicitly so dispatched relayout
    /// callbacks can find the root without walking — walking via
    /// `parent(id)` would panic if any intermediate id has been
    /// removed and reused.
    pub root: RefCell<Option<NodeId>>,
    /// Dedup flag for the port's relayout scheduler: `true` while a
    /// relayout pass is queued for the next main-loop tick, back to
    /// `false` once the pass runs. Per-tree state replaces what used
    /// to be a global thread-local HashSet — same dedup semantics,
    /// no shutdown-order vulnerability.
    pub relayout_queued: Cell<bool>,
}

pub type TreeRef<B> = Rc<LayoutTree<B>>;

/// What an element's layout slot stores.
///
/// Embedded by each port's element wrapper. While `handle` is
/// `None`, style mutations stay local; once `handle` is `Some`,
/// they're also pushed into the tree.
///
/// `meta` is the per-node backend metadata that gets copied into
/// the tree on registration. Set it before `register_in_tree`; once
/// the node has joined a tree, the slot's copy and the tree's copy
/// can drift if you mutate the slot directly. Use
/// [`LayoutTree::set_meta`] to update a registered node's metadata.
#[derive(Debug)]
pub struct NodeLayout<B: LayoutBackend> {
    pub style: Style,
    pub handle: Option<LayoutHandle<B>>,
    pub meta: B::NodeMeta,
}

impl<B: LayoutBackend> NodeLayout<B> {
    pub fn new(style: Style) -> Self {
        NodeLayout {
            style,
            handle: None,
            meta: Default::default(),
        }
    }
}

/// Where a node lives once it's joined a tree.
pub struct LayoutHandle<B: LayoutBackend> {
    pub tree: TreeRef<B>,
    pub node_id: NodeId,
}

impl<B: LayoutBackend> Clone for LayoutHandle<B> {
    fn clone(&self) -> Self {
        LayoutHandle {
            tree: self.tree.clone(),
            node_id: self.node_id,
        }
    }
}

impl<B: LayoutBackend> std::fmt::Debug for LayoutHandle<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutHandle")
            .field("node_id", &self.node_id)
            .finish()
    }
}

// ---------------------------------------------------------------------
// Internal storage
// ---------------------------------------------------------------------

struct LayoutState<B: LayoutBackend> {
    nodes: SlotMap<DefaultKey, NodeData<B>>,
}

impl<B: LayoutBackend> Default for LayoutState<B> {
    fn default() -> Self {
        LayoutState { nodes: SlotMap::new() }
    }
}

struct NodeData<B: LayoutBackend> {
    style: Style,
    cache: Cache,
    unrounded_layout: Layout,
    final_layout: Layout,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    view: B::View,
    meta: B::NodeMeta,
}

impl<B: LayoutBackend> NodeData<B> {
    fn new(style: Style, view: B::View, meta: B::NodeMeta) -> Self {
        NodeData {
            style,
            cache: Cache::new(),
            unrounded_layout: Layout::new(),
            final_layout: Layout::new(),
            parent: None,
            children: Vec::new(),
            view,
            meta,
        }
    }
}

fn key(id: NodeId) -> DefaultKey {
    id.into()
}

// ---------------------------------------------------------------------
// LayoutTree public API
// ---------------------------------------------------------------------

impl<B: LayoutBackend> LayoutTree<B> {
    /// Construct a fresh, empty tree. Returns an `Rc` so registered
    /// nodes can carry a tree reference.
    pub fn new() -> TreeRef<B> {
        Rc::new(LayoutTree {
            state: RefCell::new(LayoutState::default()),
            root: RefCell::new(None),
            relayout_queued: Cell::new(false),
        })
    }

    // -- internal helpers --------------------------------------------

    fn with_node<R>(&self, id: NodeId, f: impl FnOnce(&NodeData<B>) -> R) -> Option<R> {
        self.state.borrow().nodes.get(key(id)).map(f)
    }

    fn with_state_mut<R>(&self, f: impl FnOnce(&mut LayoutState<B>) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    /// Run a Taffy layout pass (`compute_root_layout` + `round_layout`)
    /// rooted at `id` with the given available space.
    pub fn run_layout_pass(&self, id: NodeId, available_space: Size<AvailableSpace>) {
        self.with_state_mut(|s| {
            compute_root_layout(s, id, available_space);
            round_layout(s, id);
        });
    }

    // -- mutation -----------------------------------------------------

    /// Insert a fresh leaf, returning its `NodeId`.
    pub fn new_leaf(&self, style: Style, view: B::View, meta: B::NodeMeta) -> NodeId {
        NodeId::from(
            self.with_state_mut(|s| s.nodes.insert(NodeData::new(style, view, meta))),
        )
    }

    /// Remove a node from the tree. Detaches it from any parent it
    /// was under; orphans its descendants. Callers typically remove
    /// leaves first today.
    ///
    /// Marks the (former) parent dirty so its cached flex layout
    /// is invalidated — without this, `compute_layout` would return
    /// a stale parent layout that still references the removed child.
    pub fn remove(&self, id: NodeId) {
        let parent = self.with_state_mut(|s| {
            let Some((parent, kids)) = s
                .nodes
                .get(key(id))
                .map(|n| (n.parent, n.children.clone()))
            else {
                return None;
            };
            if let Some(p) = parent {
                if let Some(p_data) = s.nodes.get_mut(key(p)) {
                    p_data.children.retain(|c| *c != id);
                }
            }
            for c in kids {
                if let Some(c_data) = s.nodes.get_mut(key(c)) {
                    c_data.parent = None;
                }
            }
            s.nodes.remove(key(id));
            parent
        });
        if let Some(p) = parent {
            self.mark_dirty(p);
        }
    }

    /// Add a child edge. Idempotent: if `child` is already under
    /// `parent`, no-op.
    ///
    /// If `child` is currently a child of a *different* parent, it's
    /// detached from that one first — same invariant as `addSubview:`
    /// on AppKit/UIKit, where a view can only have one superview.
    pub fn add_child(&self, parent: NodeId, child: NodeId) {
        let changed = self.with_state_mut(|s| {
            // Detach from previous parent if it was somewhere else.
            let prev_parent = s.nodes.get(key(child)).and_then(|c| c.parent);
            if prev_parent == Some(parent) {
                let already = s
                    .nodes
                    .get(key(parent))
                    .map(|p| p.children.contains(&child))
                    .unwrap_or(false);
                if already {
                    return None;
                }
            } else if let Some(prev) = prev_parent {
                if let Some(p_data) = s.nodes.get_mut(key(prev)) {
                    p_data.children.retain(|c| *c != child);
                }
            }
            if let Some(p) = s.nodes.get_mut(key(parent)) {
                if !p.children.contains(&child) {
                    p.children.push(child);
                }
            }
            if let Some(c) = s.nodes.get_mut(key(child)) {
                c.parent = Some(parent);
            }
            Some(prev_parent)
        });
        match changed {
            None => {} // no-op, no dirty
            Some(prev) => {
                self.mark_dirty(parent);
                if let Some(prev) = prev {
                    if prev != parent {
                        self.mark_dirty(prev);
                    }
                }
            }
        }
    }

    /// Place `child` at exactly `idx` under `parent`. If `child` was
    /// already a child of `parent` at a different index, it's moved.
    /// If `child` was a child of a *different* parent, it's detached
    /// from there first.
    pub fn insert_child_at_index(&self, parent: NodeId, idx: usize, child: NodeId) {
        let prev_parent = self.with_state_mut(|s| {
            let prev = s.nodes.get(key(child)).and_then(|c| c.parent);
            if prev != Some(parent) {
                if let Some(prev) = prev {
                    if let Some(p_data) = s.nodes.get_mut(key(prev)) {
                        p_data.children.retain(|c| *c != child);
                    }
                }
            }
            if let Some(p) = s.nodes.get_mut(key(parent)) {
                p.children.retain(|c| *c != child);
                let i = idx.min(p.children.len());
                p.children.insert(i, child);
            }
            if let Some(c) = s.nodes.get_mut(key(child)) {
                c.parent = Some(parent);
            }
            prev
        });
        self.mark_dirty(parent);
        if let Some(prev) = prev_parent {
            if prev != parent {
                self.mark_dirty(prev);
            }
        }
    }

    pub fn remove_child(&self, parent: NodeId, child: NodeId) {
        self.with_state_mut(|s| {
            if let Some(p) = s.nodes.get_mut(key(parent)) {
                p.children.retain(|c| *c != child);
            }
            if let Some(c) = s.nodes.get_mut(key(child)) {
                if c.parent == Some(parent) {
                    c.parent = None;
                }
            }
        });
        self.mark_dirty(parent);
    }

    /// Mark `id` and its ancestors dirty (cleared cache → forced
    /// re-layout). Stops walking upward as soon as it hits a node
    /// that's already dirty.
    pub fn mark_dirty(&self, id: NodeId) {
        self.with_state_mut(|s| {
            let mut current = Some(id);
            while let Some(c) = current {
                let Some(node) = s.nodes.get_mut(key(c)) else { break };
                if node.cache.is_empty() {
                    break;
                }
                node.cache.clear();
                current = node.parent;
            }
        });
    }

    pub fn set_style(&self, id: NodeId, style: Style) {
        self.with_state_mut(|s| {
            if let Some(node) = s.nodes.get_mut(key(id)) {
                node.style = style;
            }
        });
        self.mark_dirty(id);
    }

    /// Override the final (rounded) layout for a node. Used by ports
    /// that run a second compute pass on a subtree (e.g. cocoa's
    /// scroll-view content sizing) and want the node's own final
    /// layout to reflect the *first*-pass result.
    pub fn set_final_layout(&self, id: NodeId, layout: Layout) {
        self.with_state_mut(|s| {
            if let Some(node) = s.nodes.get_mut(key(id)) {
                node.final_layout = layout;
            }
        });
    }

    /// Update a node's port-specific meta. Useful for flags whose
    /// state changes after the node has been registered (rare, but
    /// happens for is_scroll_view set by the cocoa builder *after*
    /// `Element::create_with`).
    pub fn set_meta(&self, id: NodeId, meta: B::NodeMeta) {
        self.with_state_mut(|s| {
            if let Some(node) = s.nodes.get_mut(key(id)) {
                node.meta = meta;
            }
        });
    }

    // -- read accessors -----------------------------------------------

    /// Final (rounded) layout. `apply_layout` consumes this.
    pub fn layout(&self, id: NodeId) -> Option<Layout> {
        self.with_node(id, |n| n.final_layout)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.with_node(id, |n| n.parent).flatten()
    }

    /// Borrowed view of `id`'s children. Returns an empty slice if
    /// the node isn't in the tree.
    ///
    /// The returned [`Ref`] holds a shared borrow on the tree's
    /// internal storage for its lifetime — fine for plain iteration,
    /// but if you need to call mutating methods on the tree (`set_*`,
    /// `add_child`, `run_layout_pass`, …) during the loop body,
    /// `to_vec()` first to release the borrow:
    ///
    /// ```ignore
    /// // Read-only walk: zero-copy.
    /// for &child in tree.children(id).iter() { … }
    ///
    /// // Recursion that may mutate: collect first.
    /// let kids: Vec<_> = tree.children(id).to_vec();
    /// for child in kids { tree.set_style(child, …); }
    /// ```
    pub fn children(&self, id: NodeId) -> Ref<'_, [NodeId]> {
        Ref::map(self.state.borrow(), |s| {
            s.nodes
                .get(key(id))
                .map(|n| n.children.as_slice())
                .unwrap_or(&[])
        })
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

    /// Cheap accessor for the platform view; equivalent to
    /// `get_node_context(id).map(|c| c.view)` but skips the meta
    /// clone.
    pub fn view(&self, id: NodeId) -> Option<B::View> {
        self.with_node(id, |n| n.view.clone())
    }

    /// Cheap accessor for the per-node backend metadata. Used by
    /// scroll-view detection and similar dispatch where pulling the
    /// `View` clone is wasted work.
    pub fn meta(&self, id: NodeId) -> Option<B::NodeMeta> {
        self.with_node(id, |n| n.meta.clone())
    }

    /// Walk the subtree rooted at `id` in pre-order and call
    /// `visitor(node_id, layout, view)` for each node that has both
    /// a stored layout and a view. Used by ports whose frame-
    /// application strategy is "set every node's frame" (cocoa, iOS).
    /// GTK doesn't use this — its allocate cycle drives recursion
    /// itself.
    pub fn walk_subtree(
        &self,
        id: NodeId,
        visitor: &mut impl FnMut(NodeId, Layout, B::View),
    ) {
        if let (Some(layout), Some(view)) = (self.layout(id), self.view(id)) {
            visitor(id, layout, view);
        }
        // Snapshot children before recursing so the visitor can call
        // mutating tree methods (e.g. `set_style`) without conflicting
        // with the outstanding `Ref` from `children()`.
        let kids = self.children(id).to_vec();
        for child in kids {
            self.walk_subtree(child, visitor);
        }
    }

}

// ---------------------------------------------------------------------
// Taffy trait impls — operate on `LayoutState<B>` so the public
// `LayoutTree<B>` can keep its `RefCell` discipline.
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
                    // Leaf: ask `compute_leaf_layout` to do the size
                    // accounting (clamping, min/max, content-box
                    // adjustments), but inject baselines from the
                    // backend so flexbox `align_items: Baseline` gets
                    // real data.
                    //
                    // We hold an immutable borrow on `this.nodes[k]`
                    // across the call — `compute_leaf_layout`'s
                    // `MeasureFunction` is `FnOnce` and runs
                    // synchronously, so the closure's borrow of
                    // `node.view` can't outlive this call. Avoids
                    // cloning style + view per leaf measure.
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
