//! Taffy-based layout for the GTK port.
//!
//! Mirrors `cocoa_dom::layout` in shape: each window owns a single
//! [`TaffyTree`], and every [`Node`](crate::Node) carries a layout
//! slot ([`NodeLayout`], shared via `Rc<RefCell<...>>`) holding its
//! current style and (once registered) its [`LayoutHandle`] inside
//! that tree.
//!
//! Where cocoa needs an explicit `compute_layout` on every dirtying
//! event, GTK does it for us: when a widget changes size/content it
//! calls `gtk_widget_queue_resize`, which propagates up and triggers
//! a fresh measure/allocate pass on the next frame. Our [`TaffyLayout`]
//! `LayoutManager` (in [`super::taffy_layout`]) is what GTK runs that
//! pass through, and it's the single place where Taffy's
//! `compute_layout` actually executes.
//!
//! Layout is computed *only at the tree root* (the window's content
//! root). Layout managers attached to nested containers don't compute
//! anything themselves — they just look up their direct children's
//! pre-computed Taffy frames and call [`gtk::Widget::allocate`].

use crate::node::Node;
use gtk4::prelude::*;
use std::{cell::RefCell, rc::Rc};
use taffy::TaffyTree;

pub use taffy::{
    AlignItems, AvailableSpace, Dimension, FlexDirection, FlexWrap,
    JustifyContent, Layout, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Size, Style,
};

/// Per-Taffy-node user data. Attaches the underlying `gtk::Widget` so
/// the leaf measure callback can call `widget.measure(...)` for
/// content-driven sizing.
#[derive(Clone)]
pub struct NodeContext {
    pub widget: gtk4::Widget,
}

/// Owns a Taffy tree plus a slot for the tree's root NodeId. Created
/// once per [`Window`](crate::window); each node registered into the
/// window borrows a clone (Rc-bumped) of this handle so it can address
/// its own slot in the tree later.
pub struct LayoutTree {
    pub tree: RefCell<TaffyTree<NodeContext>>,
    pub root: RefCell<Option<NodeId>>,
}

pub type TreeRef = Rc<LayoutTree>;

/// Construct a fresh, empty Taffy tree wrapped for sharing.
pub fn new_tree() -> TreeRef {
    Rc::new(LayoutTree {
        tree: RefCell::new(TaffyTree::new()),
        root: RefCell::new(None),
    })
}

/// What a [`Node`] knows about its layout state. Lives behind an
/// `Rc<RefCell<...>>` inside the Node; clones share it.
#[derive(Debug)]
pub struct NodeLayout {
    /// The node's current style. Setters mutate this. When the node
    /// joins a tree, this is the seed used for `new_leaf`.
    pub style: Style,
    /// Set once the node has been registered in a tree.
    pub handle: Option<LayoutHandle>,
}

impl NodeLayout {
    pub fn new(style: Style) -> Self {
        NodeLayout { style, handle: None }
    }
}

/// Where a node lives once it's joined a Taffy tree.
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
// Registration
// ---------------------------------------------------------------------

/// Register `node` as a leaf in `tree` if it isn't already registered.
/// Idempotent. Uses the node's currently-stored style as the seed and
/// attaches the GTK widget as the node's Taffy context (used by the
/// measure closure during layout).
pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    let context = NodeContext {
        widget: node.widget().clone(),
    };
    let node_id = tree
        .tree
        .borrow_mut()
        .new_leaf_with_context(layout.style.clone(), context)
        .expect("taffy: new_leaf_with_context failed");
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

/// Drop the Taffy node and unregister this node. Called from
/// [`Node::teardown`] during unmount. No-op if the node was never
/// registered.
pub fn drop_node(node: &Node) {
    let handle = node.layout_slot().borrow_mut().handle.take();
    if let Some(h) = handle {
        let parent_id = h.tree.tree.borrow().parent(h.node_id);
        let _ = h.tree.tree.borrow_mut().remove(h.node_id);
        if let Some(pid) = parent_id {
            let _ = h.tree.tree.borrow_mut().mark_dirty(pid);
            queue_root_resize(&h.tree);
        }
    }
}

// ---------------------------------------------------------------------
// Edge mirroring (called from gtk_dom::node insert/remove)
// ---------------------------------------------------------------------

/// Add a Taffy parent-child edge. If the child isn't yet registered,
/// register it in the parent's tree first. If the parent isn't
/// registered, this is a no-op (the child stays orphan in Taffy too —
/// it'll be registered when the parent joins a tree).
pub fn attach_child(parent: &Node, child: &Node) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else {
        return;
    };
    register_in_tree(child, &parent_h.tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    {
        let mut tree = parent_h.tree.tree.borrow_mut();
        let existing = tree.children(parent_h.node_id).unwrap_or_default();
        if existing.iter().any(|c| *c == child_id) {
            let _ = tree.remove_child(parent_h.node_id, child_id);
        }
        let _ = tree.add_child(parent_h.node_id, child_id);
        let _ = tree.mark_dirty(parent_h.node_id);
    }
    queue_root_resize(&parent_h.tree);
}

/// Insert a Taffy child at a specific index under `parent`.
pub fn insert_child_at(parent: &Node, child: &Node, index: usize) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else {
        return;
    };
    register_in_tree(child, &parent_h.tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    {
        let mut tree = parent_h.tree.tree.borrow_mut();
        let existing = tree.children(parent_h.node_id).unwrap_or_default();
        if existing.iter().any(|c| *c == child_id) {
            let _ = tree.remove_child(parent_h.node_id, child_id);
        }
        let _ = tree.insert_child_at_index(parent_h.node_id, index, child_id);
        let _ = tree.mark_dirty(parent_h.node_id);
    }
    queue_root_resize(&parent_h.tree);
}

/// Remove a Taffy parent-child edge. No-op if either side isn't
/// registered.
pub fn detach_child(parent: &Node, child: &Node) {
    let parent_handle = parent.layout_slot().borrow().handle.clone();
    let Some(parent_h) = parent_handle else {
        return;
    };
    let child_id = match child.layout_slot().borrow().handle.as_ref() {
        Some(h) => h.node_id,
        None => return,
    };
    {
        let mut tree = parent_h.tree.tree.borrow_mut();
        let _ = tree.remove_child(parent_h.node_id, child_id);
        let _ = tree.mark_dirty(parent_h.node_id);
    }
    queue_root_resize(&parent_h.tree);
}

/// Ask GTK to re-run measure+allocate on the tree's root widget. Each
/// of our [`super::taffy_layout::TaffyLayout`] instances registers
/// itself with the tree on `root()`, so we can fish the root widget
/// out and `queue_resize` it. Works even when the root has finished
/// layout already (GTK coalesces multiple queue_resize calls).
pub fn queue_root_resize(tree: &TreeRef) {
    let Some(root_id) = *tree.root.borrow() else {
        return;
    };
    let widget = {
        let t = tree.tree.borrow();
        t.get_node_context(root_id).map(|c| c.widget.clone())
    };
    if let Some(w) = widget {
        w.queue_resize();
    }
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

/// Apply a function to the node's stored style and (if registered)
/// push the updated style into its tree.
pub fn update_style(node: &Node, f: impl FnOnce(&mut Style)) {
    let mut layout = node.layout_slot().borrow_mut();
    f(&mut layout.style);
    if let Some(h) = &layout.handle {
        let _ = h.tree.tree.borrow_mut().set_style(h.node_id, layout.style.clone());
    }
}

/// Replace the node's style entirely.
pub fn set_style(node: &Node, style: Style) {
    update_style(node, |s| *s = style);
}

/// Mark this node dirty in Taffy and queue a GTK resize. Call after
/// content changes that affect intrinsic size (button title, label
/// text, etc.). GTK already calls `queue_resize` for us when changing
/// the corresponding widget property — but we still need to mark
/// Taffy dirty so the cached measurement is invalidated.
pub fn schedule_relayout(node: &Node) {
    let handle = node.layout_slot().borrow().handle.clone();
    if let Some(h) = handle {
        let _ = h.tree.tree.borrow_mut().mark_dirty(h.node_id);
        queue_root_resize(&h.tree);
    }
}

// ---------------------------------------------------------------------
// Convenience setters for common style properties.
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
        s.padding = taffy::Rect {
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

pub fn set_margin(node: &Node, all_px: f32) {
    update_style(node, |s| {
        s.margin = taffy::Rect {
            left: LengthPercentageAuto::length(all_px),
            right: LengthPercentageAuto::length(all_px),
            top: LengthPercentageAuto::length(all_px),
            bottom: LengthPercentageAuto::length(all_px),
        };
    });
    schedule_relayout(node);
}

// ---------------------------------------------------------------------
// compute_layout (test helper)
// ---------------------------------------------------------------------
//
// At runtime, layout is driven by GTK's measure/allocate cycle through
// our `taffy_layout::TaffyLayout` LayoutManager. For unit tests we want
// to be able to compute layout against a fixed `available_size` without
// running a GTK main loop. This helper mirrors cocoa_dom's
// `layout::compute_layout(root, size)` shape.

/// Compute Taffy layout for the subtree rooted at `root` against the
/// given available size. Forces the root to fill the size exactly,
/// then runs `compute_layout_with_measure` so leaf measure callbacks
/// fire for content-driven sizing.
///
/// `root` must be registered in a tree.
pub fn compute_layout(root: &Node, available_size: (f32, f32)) {
    let handle = root.layout_slot().borrow().handle.clone();
    let Some(handle) = handle else {
        return;
    };

    let (w, h) = available_size;
    let mut tree = handle.tree.tree.borrow_mut();

    let mut style = tree.style(handle.node_id).cloned().unwrap_or_default();
    style.size = Size {
        width: Dimension::length(w),
        height: Dimension::length(h),
    };
    let _ = tree.set_style(handle.node_id, style);

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    let _ = tree.compute_layout_with_measure(
        handle.node_id,
        avail,
        measure_closure,
    );
}

// ---------------------------------------------------------------------
// Measure callback for leaf widgets.
// ---------------------------------------------------------------------

/// Reusable function-pointer used by `compute_layout_with_measure` so
/// both call sites share a single monomorphization.
pub fn measure_closure(
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
    _node_id: NodeId,
    ctx: Option<&mut NodeContext>,
    _style: &Style,
) -> Size<f32> {
    if let (Some(w), Some(h)) = (known.width, known.height) {
        return Size { width: w, height: h };
    }
    let Some(ctx) = ctx else {
        return Size {
            width: known.width.unwrap_or(0.0),
            height: known.height.unwrap_or(0.0),
        };
    };

    let widget = &ctx.widget;

    // GTK widgets that report "constant size for any orientation" are
    // either handled by their layout manager (containers — Taffy
    // descends into them; we don't measure them) or are leaves whose
    // intrinsic size we can ask for via measure(orientation, -1).
    //
    // For an editable Entry: the natural width tracks content (grows
    // as you type), which would push the field's frame outwards on
    // every keystroke. Force width=0 so the parent's flex layout
    // decides — same trick cocoa_dom's measure_leaf does for
    // NSTextField.

    let constraint_w = match (known.width, avail.width) {
        (Some(w), _) => Some(w as i32),
        (_, AvailableSpace::Definite(w)) => Some(w as i32),
        _ => None,
    };

    let (_, nat_w, _, _) = if let Some(h) = known.height {
        widget.measure(gtk4::Orientation::Horizontal, h as i32)
    } else {
        widget.measure(gtk4::Orientation::Horizontal, -1)
    };
    let mut w = known.width.unwrap_or(nat_w as f32);

    // For editable text entries, the natural width tracks content
    // (the entry grows as the user types), which would push the
    // field's frame outwards on every keystroke. Force width=0 so
    // the parent's flex layout decides — same trick cocoa_dom's
    // measure_leaf does for editable NSTextField. Sliders and
    // dropdowns have stable natural widths, so we leave them alone
    // (cocoa parity: NSSlider and NSPopUpButton keep their
    // intrinsic widths there too).
    if widget.is::<gtk4::Entry>() || widget.is::<gtk4::PasswordEntry>() {
        if known.width.is_none() {
            w = 0.0;
        }
    }

    let height_for = constraint_w.unwrap_or(w as i32).max(-1);
    let (_, nat_h, _, _) =
        widget.measure(gtk4::Orientation::Vertical, height_for);
    let h = known.height.unwrap_or(nat_h as f32);

    Size {
        width: w.max(0.0),
        height: h.max(0.0),
    }
}
