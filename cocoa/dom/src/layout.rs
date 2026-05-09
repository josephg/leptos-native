//! Layout engine — port-owned tree with Taffy as the layout algorithm.
//!
//! Each cocoa_dom [`Node`] carries a shared "layout slot"
//! ([`NodeLayout`], stored in an `Rc<RefCell<...>>` shared across
//! Node clones). The slot has two pieces:
//!
//!  - the node's *current* style ([`Style`]), mutated by setters and
//!    used as the seed when the node is registered in a tree;
//!  - an `Option<LayoutHandle>` — `Some` once the node has been
//!    registered into a [`LayoutTree`] (i.e. mounted somewhere under
//!    a [`Window`](crate::window)). While `None`, style mutations
//!    stay local; once `Some`, they're also pushed into the tree.
//!
//! The tree itself stores each node's style/cache/layout/parent/
//! children/view directly — no `TaffyTree` in between. We implement
//! Taffy's `LayoutPartialTree` family of traits on the storage so
//! the public layout algorithms (`compute_root_layout`,
//! `compute_flexbox_layout`, etc.) operate on it. This is the path
//! Taffy documents for embedders that want to bring their own
//! storage; the upside for us is that
//! [`LayoutPartialTree::compute_child_layout`] is *ours*, so we can
//! return real first-text-baselines from leaves and let Taffy's
//! flexbox engine do `align_items: Baseline` properly. (The public
//! `compute_layout_with_measure` API on `TaffyTree` discards
//! baselines; the leaf path hardcodes `first_baselines: Point::NONE`
//! before flexbox sees the value.)

use crate::node::Node;
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSControl, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use send_wrapper::SendWrapper;
use slotmap::{DefaultKey, SlotMap};
use std::{cell::RefCell, rc::Rc, sync::OnceLock};

pub use taffy::{
    AlignItems, AvailableSpace, Dimension, FlexDirection, FlexWrap,
    JustifyContent, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Size, Style,
};
#[cfg(feature = "block_layout")]
pub use taffy::Display;
use taffy::{
    compute_cached_layout, compute_flexbox_layout, compute_hidden_layout,
    compute_leaf_layout, compute_root_layout, round_layout, Cache, CacheTree,
    Display as TaffyDisplay, Layout, LayoutFlexboxContainer, LayoutInput,
    LayoutOutput, LayoutPartialTree, Point, Rect, RoundTree,
    TraversePartialTree, TraverseTree,
};

#[cfg(feature = "block_layout")]
use taffy::{compute_block_layout, LayoutBlockContainer};

/// Toggle layout debug output by setting the `COCOA_DOM_LAYOUT_DEBUG`
/// environment variable to any value (e.g.
/// `COCOA_DOM_LAYOUT_DEBUG=1 cargo run ...`).
fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("COCOA_DOM_LAYOUT_DEBUG").is_some())
}

// ---------------------------------------------------------------------
// Public types (kept for backwards compatibility with callers that
// still spell things in TaffyTree-shaped names).
// ---------------------------------------------------------------------

/// Lightweight read-only view of a node's per-tree context. Kept as a
/// public type because external callers (`debug_overlay`,
/// `renderer_cocoa`) still reach for `tree.get_node_context(...)`.
#[derive(Clone)]
pub struct NodeContext {
    pub view: SendWrapper<Retained<NSView>>,
    /// True if this node backs an `<scroll_view>` (NSScrollView).
    /// Triggers the scroll-view second pass.
    pub is_scroll_view: bool,
}

/// Owns the layout tree plus a slot for the tree's root NodeId.
/// Created once per [`Window`](crate::window); each registered node
/// keeps an Rc clone so late-firing reactive effects can address the
/// right tree.
pub struct LayoutTree {
    state: RefCell<LayoutState>,
    /// Set by `register_in_tree` the first time a tree gets a node.
    /// Tracked explicitly so `schedule_relayout_for_tree` can find
    /// the root without walking — walking via `parent(id)` would
    /// panic if any intermediate id has been removed and reused.
    pub root: RefCell<Option<NodeId>>,
}

pub type TreeRef = Rc<LayoutTree>;

pub fn new_tree() -> TreeRef {
    Rc::new(LayoutTree {
        state: RefCell::new(LayoutState::default()),
        root: RefCell::new(None),
    })
}

/// What a [`Node`] knows about its layout state.
#[derive(Debug)]
pub struct NodeLayout {
    pub style: Style,
    pub handle: Option<LayoutHandle>,
    pub is_scroll_view: bool,
}

impl NodeLayout {
    pub fn new(style: Style) -> Self {
        NodeLayout { style, handle: None, is_scroll_view: false }
    }
}

/// Where a node lives once it's joined a tree.
#[derive(Clone)]
pub struct LayoutHandle {
    pub tree: TreeRef,
    pub node_id: NodeId,
}

impl std::fmt::Debug for LayoutHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutHandle")
            .field("node_id", &self.node_id)
            .finish()
    }
}

// ---------------------------------------------------------------------
// Internal storage
// ---------------------------------------------------------------------

#[derive(Default)]
struct LayoutState {
    nodes: SlotMap<DefaultKey, NodeData>,
}

struct NodeData {
    style: Style,
    cache: Cache,
    unrounded_layout: Layout,
    final_layout: Layout,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    view: SendWrapper<Retained<NSView>>,
    is_scroll_view: bool,
}

impl NodeData {
    fn new(style: Style, view: SendWrapper<Retained<NSView>>, is_scroll_view: bool) -> Self {
        NodeData {
            style,
            cache: Cache::new(),
            unrounded_layout: Layout::new(),
            final_layout: Layout::new(),
            parent: None,
            children: Vec::new(),
            view,
            is_scroll_view,
        }
    }
}

fn key(id: NodeId) -> DefaultKey {
    id.into()
}

// ---------------------------------------------------------------------
// LayoutTree public API (RefCell-wrapped accessors)
// ---------------------------------------------------------------------

impl LayoutTree {
    // -- internal access helpers --------------------------------------

    /// Borrow the state and run `f` against the node's data, if it
    /// exists.
    fn with_node<R>(&self, id: NodeId, f: impl FnOnce(&NodeData) -> R) -> Option<R> {
        self.state.borrow().nodes.get(key(id)).map(f)
    }

    /// Borrow the state mutably and run `f`.
    fn with_state_mut<R>(&self, f: impl FnOnce(&mut LayoutState) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    /// Run a Taffy layout pass (`compute_root_layout` + `round_layout`)
    /// against `id` with `available_space`.
    fn run_layout_pass(&self, id: NodeId, available_space: Size<AvailableSpace>) {
        self.with_state_mut(|s| {
            compute_root_layout(s, id, available_space);
            round_layout(s, id);
        });
    }

    // -- mutation -----------------------------------------------------

    /// Insert a fresh leaf, returning its NodeId.
    fn new_leaf_with_context(
        &self,
        style: Style,
        view: SendWrapper<Retained<NSView>>,
        is_scroll_view: bool,
    ) -> NodeId {
        NodeId::from(self.with_state_mut(|s| {
            s.nodes.insert(NodeData::new(style, view, is_scroll_view))
        }))
    }

    /// Remove a node from the tree. Detaches it from any parent it
    /// was under; orphans its descendants (callers always remove
    /// leaves first today).
    fn remove(&self, id: NodeId) {
        self.with_state_mut(|s| {
            let Some((parent, kids)) = s
                .nodes
                .get(key(id))
                .map(|n| (n.parent, n.children.clone()))
            else {
                return;
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
        });
    }

    /// Add a child edge. If the edge already exists, no-op (the
    /// existing position is preserved — Mountable cascades visit
    /// children in construction order, which is the order we already
    /// have, so duplicate-detect-and-keep matches what NSView's
    /// `addSubview` does for already-mounted children in practice).
    fn add_child(&self, parent: NodeId, child: NodeId) {
        self.with_state_mut(|s| {
            if let Some(p) = s.nodes.get_mut(key(parent)) {
                if p.children.contains(&child) {
                    return;
                }
                p.children.push(child);
            }
            if let Some(c) = s.nodes.get_mut(key(child)) {
                c.parent = Some(parent);
            }
        });
        self.mark_dirty(parent);
    }

    /// Place `child` at exactly `idx` under `parent`. If `child` was
    /// already a child of `parent` at a different index, it's moved.
    fn insert_child_at_index(&self, parent: NodeId, idx: usize, child: NodeId) {
        self.with_state_mut(|s| {
            if let Some(p) = s.nodes.get_mut(key(parent)) {
                p.children.retain(|c| *c != child);
                let i = idx.min(p.children.len());
                p.children.insert(i, child);
            }
            if let Some(c) = s.nodes.get_mut(key(child)) {
                c.parent = Some(parent);
            }
        });
        self.mark_dirty(parent);
    }

    fn remove_child(&self, parent: NodeId, child: NodeId) {
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
    /// that's already dirty — its ancestors were dirty when it was
    /// dirtied, so they're still dirty.
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

    // -- read accessors -----------------------------------------------

    /// Final (rounded) layout. `apply_layout` consumes this.
    pub fn layout(&self, id: NodeId) -> Option<Layout> {
        self.with_node(id, |n| n.final_layout)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.with_node(id, |n| n.parent).flatten()
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.with_node(id, |n| n.children.clone()).unwrap_or_default()
    }

    pub fn dirty(&self, id: NodeId) -> bool {
        self.with_node(id, |n| n.cache.is_empty()).unwrap_or(true)
    }

    pub fn get_node_context(&self, id: NodeId) -> Option<NodeContext> {
        self.with_node(id, |n| NodeContext {
            view: n.view.clone(),
            is_scroll_view: n.is_scroll_view,
        })
    }

    pub fn style(&self, id: NodeId) -> Option<Style> {
        self.with_node(id, |n| n.style.clone())
    }
}

// ---------------------------------------------------------------------
// Taffy trait impls on LayoutState
// ---------------------------------------------------------------------

impl TraversePartialTree for LayoutState {
    type ChildIter<'a> = std::iter::Copied<std::slice::Iter<'a, NodeId>>;

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

impl TraverseTree for LayoutState {}

impl CacheTree for LayoutState {
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

impl LayoutPartialTree for LayoutState {
    type CoreContainerStyle<'a> = &'a Style where Self: 'a;
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
                #[cfg(feature = "block_layout")]
                (TaffyDisplay::Block, true) => {
                    compute_block_layout(this, id, inputs, None)
                }
                (TaffyDisplay::Flex, true) => compute_flexbox_layout(this, id, inputs),
                (_, false) => {
                    // Leaf — measure size, query baseline, build the
                    // output. `compute_leaf_layout` does the size
                    // accounting (clamping, min/max, content-box
                    // adjustments) and returns a `LayoutOutput` with
                    // `first_baselines: NONE`. We then overwrite the
                    // baseline with our port-corrected value so
                    // `align_items: Baseline` works in flexbox.
                    let style = this.nodes[k].style.clone();
                    let view: Retained<NSView> = (*this.nodes[k].view).clone();
                    let mut out = compute_leaf_layout(
                        inputs,
                        &style,
                        |_, _| 0.0,
                        |known, avail| measure_leaf_size(known, avail, &view),
                    );
                    out.first_baselines.y =
                        first_baseline_offset(&view).map(|b| b as f32);
                    out
                }
            }
        })
    }
}

impl LayoutFlexboxContainer for LayoutState {
    type FlexboxContainerStyle<'a> = &'a Style where Self: 'a;
    type FlexboxItemStyle<'a> = &'a Style where Self: 'a;

    fn get_flexbox_container_style(&self, id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        &self.nodes[key(id)].style
    }

    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.nodes[key(id)].style
    }
}

#[cfg(feature = "block_layout")]
impl LayoutBlockContainer for LayoutState {
    type BlockContainerStyle<'a> = &'a Style where Self: 'a;
    type BlockItemStyle<'a> = &'a Style where Self: 'a;

    fn get_block_container_style(&self, id: NodeId) -> Self::BlockContainerStyle<'_> {
        &self.nodes[key(id)].style
    }

    fn get_block_child_style(&self, id: NodeId) -> Self::BlockItemStyle<'_> {
        &self.nodes[key(id)].style
    }
}

impl RoundTree for LayoutState {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.nodes[key(id)].unrounded_layout
    }

    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.nodes[key(id)].final_layout = *layout;
    }
}

// ---------------------------------------------------------------------
// Registration / teardown
// ---------------------------------------------------------------------

/// Register `node` as a leaf in `tree` if not already registered.
pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    let view: Retained<NSView> = node.ns_view().into();
    let view_wrapped = SendWrapper::new(view);
    let node_id = tree.new_leaf_with_context(
        layout.style.clone(),
        view_wrapped,
        layout.is_scroll_view,
    );
    {
        let mut root = tree.root.borrow_mut();
        if root.is_none() {
            *root = Some(node_id);
        }
    }
    layout.handle = Some(LayoutHandle {
        tree: tree.clone(),
        node_id,
    });
}

/// Drop the node and unregister it. No-op if never registered.
pub fn drop_node(node: &Node) {
    let handle = node.layout_slot().borrow_mut().handle.take();
    if let Some(h) = handle {
        let parent_id = h.tree.parent(h.node_id);
        h.tree.remove(h.node_id);
        if let Some(pid) = parent_id {
            h.tree.mark_dirty(pid);
            schedule_relayout_for_tree(&h.tree, pid);
        }
    }
}

// ---------------------------------------------------------------------
// Dynamic relayout — coalesce mutation bursts into one pass per tick.
// ---------------------------------------------------------------------

thread_local! {
    static PENDING: RefCell<std::collections::HashSet<usize>> =
        RefCell::new(std::collections::HashSet::new());
}

/// Schedule a re-layout of the tree this node belongs to.
///
/// Also marks the node dirty — without this, the cache returns the
/// previously-computed output and the measure callback isn't
/// re-invoked. Required for content changes on leaf controls (label
/// text, button title, etc.).
pub fn schedule_relayout(node: &Node) {
    let handle = node.layout_slot().borrow().handle.clone();
    if let Some(h) = handle {
        h.tree.mark_dirty(h.node_id);
        schedule_relayout_for_tree(&h.tree, h.node_id);
    }
}

fn schedule_relayout_for_tree(tree: &TreeRef, _any_node_id: NodeId) {
    let key = Rc::as_ptr(tree) as usize;
    let just_inserted = PENDING.with_borrow_mut(|p| p.insert(key));
    if !just_inserted {
        return;
    }
    let tree_weak = SendWrapper::new(Rc::downgrade(tree));
    DispatchQueue::main().exec_async(move || {
        let weak = tree_weak.take();
        let Some(tree) = weak.upgrade() else { return };

        PENDING.with_borrow_mut(|p| {
            p.remove(&(Rc::as_ptr(&tree) as usize));
        });

        let Some(root_id) = *tree.root.borrow() else { return };
        let root_view: Retained<NSView> = {
            let Some(ctx) = tree.get_node_context(root_id) else { return };
            (*ctx.view).clone()
        };

        let root_handle = LayoutHandle {
            tree: tree.clone(),
            node_id: root_id,
        };
        let root_node = crate::node::Node::from_view_with_handle(
            root_view.clone(),
            crate::node::NodeKind::Element,
            root_handle,
        );
        let size = root_view.frame().size;
        compute_layout(&root_node, size);
    });
}

// ---------------------------------------------------------------------
// Tree-edge mirroring
// ---------------------------------------------------------------------

pub fn attach_child(parent: &Node, child: &Node) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else { return };
    register_in_tree(child, &parent_h.tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    parent_h.tree.add_child(parent_h.node_id, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
}

pub fn insert_child_at(parent: &Node, child: &Node, index: usize) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else { return };
    register_in_tree(child, &parent_h.tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    parent_h.tree.insert_child_at_index(parent_h.node_id, index, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
}

pub fn detach_child(parent: &Node, child: &Node) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else { return };
    let child_id = match child.layout_slot().borrow().handle.as_ref() {
        Some(h) => h.node_id,
        None => return,
    };
    parent_h.tree.remove_child(parent_h.node_id, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

pub fn update_style(node: &Node, f: impl FnOnce(&mut Style)) {
    let mut layout = node.layout_slot().borrow_mut();
    f(&mut layout.style);
    if let Some(h) = &layout.handle {
        h.tree.set_style(h.node_id, layout.style.clone());
    }
}

pub fn set_style(node: &Node, style: Style) {
    update_style(node, |s| *s = style);
}

// ---------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------

/// Compute layout for the subtree rooted at `root`, then walk it and
/// assign frames to each NSView.
pub fn compute_layout(root: &Node, available_size: NSSize) {
    if layout_debug_enabled() {
        eprintln!(
            "[compute_layout] avail {:.0}x{:.0}",
            available_size.width, available_size.height
        );
    }
    let handle = root.layout_slot().borrow().handle.clone();
    let Some(handle) = handle else { return };

    let w = available_size.width as f32;
    let h = available_size.height as f32;

    // Force the root to fill the available space.
    {
        let mut style = handle.tree.style(handle.node_id).unwrap_or_default();
        style.size = Size {
            width: Dimension::length(w),
            height: Dimension::length(h),
        };
        handle.tree.set_style(handle.node_id, style);
    }

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    handle.tree.run_layout_pass(handle.node_id, avail);

    // Second pass for scroll views.
    let scroll_view_viewports = relayout_scroll_views(&handle.tree, handle.node_id);

    apply_layout(
        &handle.tree,
        handle.node_id,
        root.ns_view(),
        &scroll_view_viewports,
    );

    #[cfg(feature = "debug-overlay")]
    crate::debug_overlay::mark_overlays_dirty();
}

/// Walk the tree from `node_id`. For each scroll-view, run a second
/// `compute_root_layout` with the viewport width pinned and height =
/// MaxContent. Returns a map of `scroll_view NodeId → main-pass
/// Layout` (the viewport rect, before the second pass overwrites
/// it).
fn relayout_scroll_views(
    tree: &TreeRef,
    root: NodeId,
) -> std::collections::HashMap<NodeId, Layout> {
    let mut viewports = std::collections::HashMap::new();
    relayout_scroll_views_inner(tree, root, &mut viewports);
    viewports
}

fn relayout_scroll_views_inner(
    tree: &TreeRef,
    id: NodeId,
    viewports: &mut std::collections::HashMap<NodeId, Layout>,
) {
    let is_scroll = tree
        .get_node_context(id)
        .map(|c| c.is_scroll_view)
        .unwrap_or(false);

    if is_scroll {
        let main_layout = tree.layout(id).expect("layout missing for scroll_view");
        viewports.insert(id, main_layout);

        let viewport_w = main_layout.size.width;

        // Override the scroll_view's style for the second pass so it
        // stretches to viewport width but is allowed to grow on the
        // main axis with content. After the pass, restore.
        let saved_style = tree.style(id).expect("style missing");
        let mut probe_style = saved_style.clone();
        probe_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::auto(),
        };
        tree.set_style(id, probe_style);
        tree.mark_dirty(id);

        let avail = Size {
            width: AvailableSpace::Definite(viewport_w),
            height: AvailableSpace::MaxContent,
        };
        tree.run_layout_pass(id, avail);

        tree.set_style(id, saved_style);
        tree.mark_dirty(id);
        return;
    }

    let kids = tree.children(id);
    for child in kids {
        relayout_scroll_views_inner(tree, child, viewports);
    }
}

// ---------------------------------------------------------------------
// Leaf size measurement
// ---------------------------------------------------------------------

/// Size-only measure callback for leaves. Mirrors the previous
/// `measure_leaf` logic; the baseline is queried separately by the
/// LayoutPartialTree leaf path so it can flow into `LayoutOutput`.
fn measure_leaf_size(
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
    view: &NSView,
) -> Size<f32> {
    if let (Some(w), Some(h)) = (known.width, known.height) {
        return Size { width: w, height: h };
    }

    let any: &AnyObject = view.as_ref();
    let mut measured: NSSize = if let Some(field) =
        any.downcast_ref::<NSTextField>()
    {
        let wrapping = field.cell().is_some_and(|c| c.wraps());
        let constraint_w: Option<f32> = if let Some(w) = known.width {
            Some(w)
        } else if let AvailableSpace::Definite(w) = avail.width {
            Some(w)
        } else {
            None
        };
        if wrapping && known.height.is_none() && constraint_w.is_some() {
            // AppKit's idiomatic recipe for multiline-label sizing:
            // set `preferredMaxLayoutWidth` to the width the parent
            // is going to give us, then read `intrinsicContentSize`.
            let w = constraint_w.unwrap() as f64;
            if (field.preferredMaxLayoutWidth() - w).abs() > f64::EPSILON {
                field.setPreferredMaxLayoutWidth(w);
            }
            view.intrinsicContentSize()
        } else {
            let original = view.frame();
            (field as &NSControl).sizeToFit();
            let fit = view.frame().size;
            view.setFrame(original);
            fit
        }
    } else if let Some(control) = any.downcast_ref::<NSControl>() {
        let original = view.frame();
        control.sizeToFit();
        let fit = view.frame().size;
        view.setFrame(original);
        fit
    } else {
        view.intrinsicContentSize()
    };

    // Editable text fields: width is NOT content-driven (otherwise
    // the field grows with each keystroke). Force width to 0 so the
    // parent decides via cross-axis stretch / flex_grow.
    if let Some(field) = any.downcast_ref::<NSTextField>() {
        if field.isEditable() {
            measured.width = 0.0;
        }
    }

    fn axis(known: Option<f32>, measured_v: f64) -> f32 {
        if let Some(k) = known {
            return k;
        }
        let v = measured_v as f32;
        // NSViewNoIntrinsicMetric is -1; clamp to 0.
        if v < 0.0 { 0.0 } else { v }
    }

    Size {
        width: axis(known.width, measured.width),
        height: axis(known.height, measured.height),
    }
}

// ---------------------------------------------------------------------
// Apply Taffy's computed frames onto NSViews
// ---------------------------------------------------------------------

fn apply_layout(
    tree: &TreeRef,
    id: NodeId,
    view: &NSView,
    scroll_viewports: &std::collections::HashMap<NodeId, Layout>,
) {
    // For scroll views, prefer the cached first-pass viewport layout
    // — `tree.layout()` would now return the second-pass result.
    let layout: Layout = scroll_viewports
        .get(&id)
        .copied()
        .unwrap_or_else(|| tree.layout(id).expect("layout missing for node"));
    set_frame_from_layout(view, &layout);

    let children = tree.children(id);
    if children.is_empty() {
        return;
    }

    // For `<scroll_view>` containers, our children live inside the
    // documentView (a FlippedView we install at construction).
    let scroll_doc: Option<Retained<NSView>> = {
        let is_ours = tree
            .get_node_context(id)
            .map(|c| c.is_scroll_view)
            .unwrap_or(false);
        if is_ours {
            use objc2_app_kit::NSScrollView;
            let any: &AnyObject = view.as_ref();
            any.downcast_ref::<NSScrollView>()
                .and_then(|s| s.documentView())
        } else {
            None
        }
    };
    if let Some(doc) = scroll_doc.as_ref() {
        // Bound the documentView around its children so NSScrollView
        // shows scroll bars when content overflows.
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        for child_id in children.iter() {
            let cl = tree.layout(*child_id).expect("child layout missing");
            let right = cl.location.x + cl.size.width;
            let bottom = cl.location.y + cl.size.height;
            if right > max_x { max_x = right; }
            if bottom > max_y { max_y = bottom; }
        }
        let doc_width = (max_x as f64).max(layout.size.width as f64);
        let doc_height = (max_y as f64).max(layout.size.height as f64);
        doc.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(doc_width, doc_height),
        ));
    }

    let subview_source: &NSView = scroll_doc.as_deref().unwrap_or(view);
    let subviews = subview_source.subviews();
    // Filter out subviews tagged with `OVERLAY_TAG` (the debug
    // overlay isn't registered in our tree and would otherwise
    // consume one Taffy child's index).
    let owned: Vec<_> = subviews
        .iter()
        .filter(|sv| {
            #[cfg(feature = "debug-overlay")]
            {
                if sv.tag() == crate::debug_overlay::OVERLAY_TAG {
                    return false;
                }
            }
            true
        })
        .collect();
    for (i, child_id) in children.iter().enumerate() {
        let Some(sv) = owned.get(i) else { break };
        apply_layout(tree, *child_id, sv, scroll_viewports);
    }
}

// ---------------------------------------------------------------------
// Baseline helper (used by both the leaf path and the debug overlay)
// ---------------------------------------------------------------------

/// First-baseline offset from the top of `view`'s **frame**.
///
/// `NSView::firstBaselineOffsetFromTop` already does the work, but
/// the docs are easy to misread: the value is the distance from the
/// top of the view's *alignment rectangle* to its topmost baseline,
/// not from the top of the view's frame. For controls with a focus-
/// ring or shadow inset (NSButton has both), the alignment rect
/// starts a few points inside the frame, so a caller that uses the
/// raw value directly draws the baseline that many points too high.
///
/// We add `alignmentRectInsets.top` to convert into frame-relative
/// coordinates — which is what every caller here actually wants
/// (apply_layout setFrames in frame-coords, the debug overlay
/// strokes in frame-coords).
///
/// Returns `None` for views with no measurable text baseline. The
/// `NSView` default returns 0 — that's the docs-blessed "I don't
/// have a baseline" sentinel.
pub fn first_baseline_offset(view: &NSView) -> Option<f64> {
    let raw = view.firstBaselineOffsetFromTop() as f64;
    if raw <= 0.0 {
        return None;
    }
    let insets = view.alignmentRectInsets();
    Some(raw + insets.top as f64)
}

fn set_frame_from_layout(view: &NSView, layout: &Layout) {
    let Point { x, y } = layout.location;
    let Size { width, height } = layout.size;
    if layout_debug_enabled() {
        eprintln!(
            "  [frame] {:p} <- ({:.0},{:.0}) {:.0}x{:.0}",
            view as *const _, x, y, width, height
        );
    }
    view.setFrame(NSRect::new(
        NSPoint::new(x as f64, y as f64),
        NSSize::new(width as f64, height as f64),
    ));
}

// ---------------------------------------------------------------------
// Convenience setters
// ---------------------------------------------------------------------

pub fn set_width(node: &Node, width_px: f32) {
    update_style(node, |s| s.size.width = Dimension::length(width_px));
    schedule_relayout(node);
}

pub fn set_height(node: &Node, height_px: f32) {
    update_style(node, |s| s.size.height = Dimension::length(height_px));
    schedule_relayout(node);
}

pub fn set_min_width(node: &Node, px: f32) {
    update_style(node, |s| s.min_size.width = Dimension::length(px));
    schedule_relayout(node);
}

pub fn set_max_width(node: &Node, px: f32) {
    update_style(node, |s| s.max_size.width = Dimension::length(px));
    schedule_relayout(node);
}

pub fn set_min_height(node: &Node, px: f32) {
    update_style(node, |s| s.min_size.height = Dimension::length(px));
    schedule_relayout(node);
}

pub fn set_max_height(node: &Node, px: f32) {
    update_style(node, |s| s.max_size.height = Dimension::length(px));
    schedule_relayout(node);
}

pub fn set_flex_direction(node: &Node, dir: FlexDirection) {
    update_style(node, |s| s.flex_direction = dir);
    schedule_relayout(node);
}

pub fn set_padding(node: &Node, all_px: f32) {
    update_style(node, |s| {
        s.padding = Rect {
            left: LengthPercentage::length(all_px),
            right: LengthPercentage::length(all_px),
            top: LengthPercentage::length(all_px),
            bottom: LengthPercentage::length(all_px),
        };
    });
    schedule_relayout(node);
}

pub fn set_gap(node: &Node, gap_px: f32) {
    update_style(node, |s| {
        s.gap = Size {
            width: LengthPercentage::length(gap_px),
            height: LengthPercentage::length(gap_px),
        };
    });
    schedule_relayout(node);
}

pub fn set_justify_content(node: &Node, jc: JustifyContent) {
    update_style(node, |s| s.justify_content = Some(jc));
    schedule_relayout(node);
}

pub fn set_flex_grow(node: &Node, grow: f32) {
    update_style(node, |s| s.flex_grow = grow);
    schedule_relayout(node);
}

pub fn set_flex_shrink(node: &Node, shrink: f32) {
    update_style(node, |s| s.flex_shrink = shrink);
    schedule_relayout(node);
}

pub fn set_flex_basis(node: &Node, basis_px: f32) {
    update_style(node, |s| s.flex_basis = Dimension::length(basis_px));
    schedule_relayout(node);
}

pub fn set_align_items(node: &Node, ai: AlignItems) {
    update_style(node, |s| s.align_items = Some(ai));
    schedule_relayout(node);
}

pub fn set_flex_wrap(node: &Node, fw: FlexWrap) {
    update_style(node, |s| s.flex_wrap = fw);
    schedule_relayout(node);
}

pub fn set_background_color(node: &Node, color: crate::Color) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        let ns_color = color.to_nscolor();
        layer.setBackgroundColor(Some(&ns_color.CGColor()));
    }
}

pub fn set_clip(node: &Node, clip: bool) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setMasksToBounds(clip);
    }
}

pub fn set_margin(node: &Node, all_px: f32) {
    update_style(node, |s| {
        s.margin = Rect {
            left: LengthPercentageAuto::length(all_px),
            right: LengthPercentageAuto::length(all_px),
            top: LengthPercentageAuto::length(all_px),
            bottom: LengthPercentageAuto::length(all_px),
        };
    });
    schedule_relayout(node);
}

pub fn set_align_self(node: &Node, ai: Option<AlignItems>) {
    update_style(node, |s| s.align_self = ai);
    schedule_relayout(node);
}

pub fn dim_to_dimension(d: renderer::attrs::Dim) -> Dimension {
    use renderer::attrs::Dim;
    match d {
        Dim::Px(v) => Dimension::length(v),
        Dim::Pct(v) => Dimension::percent(v),
        Dim::Auto => Dimension::auto(),
    }
}

pub fn align_self_to_taffy(
    a: renderer::attrs::AlignSelf,
) -> Option<AlignItems> {
    use renderer::attrs::AlignSelf;
    match a {
        AlignSelf::Auto => None,
        AlignSelf::Start => Some(AlignItems::FlexStart),
        AlignSelf::End => Some(AlignItems::FlexEnd),
        AlignSelf::Center => Some(AlignItems::Center),
        AlignSelf::Stretch => Some(AlignItems::Stretch),
        AlignSelf::Baseline => Some(AlignItems::Baseline),
    }
}
