//! Taffy-based layout engine.
//!
//! Each cocoa_dom [`Node`] carries a shared "layout slot"
//! ([`NodeLayout`], stored in an `Rc<RefCell<...>>` shared across
//! Node clones). The slot has two pieces:
//!
//!  - the node's *current* style ([`Style`]), mutated by setters and
//!    used as the seed when the node is registered in a tree;
//!  - an `Option<LayoutHandle>` — `Some` once the node has been
//!    registered into a [`TaffyTree`] (i.e. mounted somewhere under a
//!    [`Window`](crate::app)). While `None`, style mutations stay
//!    local; once `Some`, they're also pushed into the tree.
//!
//! Trees themselves are owned by their [`Window`]
//! (`Rc<RefCell<TaffyTree<()>>>`). Each LayoutHandle keeps an Rc to
//! its tree, so late-firing reactive effects can mutate the right
//! tree without consulting any global registry.

use crate::node::Node;
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSControl, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use send_wrapper::SendWrapper;
use std::{cell::RefCell, rc::Rc, sync::OnceLock};

/// Toggle layout debug output by setting the `COCOA_DOM_LAYOUT_DEBUG`
/// environment variable to any value (e.g.
/// `COCOA_DOM_LAYOUT_DEBUG=1 cargo run ...`).
///
/// Cached after first read — set the var before the first
/// `compute_layout` call and it's stable for the run.
fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("COCOA_DOM_LAYOUT_DEBUG").is_some())
}
pub use taffy::{
    AvailableSpace, Dimension, FlexDirection, JustifyContent, LengthPercentage,
    LengthPercentageAuto, NodeId, Position, Size, Style,
};
use taffy::{Layout, Point, TaffyTree};

/// Per-Taffy-node user data. We attach the underlying NSView so the
/// measure closure (passed to `compute_layout_with_measure`) can call
/// `NSView::intrinsicContentSize` for leaf controls (NSButton,
/// NSTextField, etc.) and Taffy can size them based on their actual
/// content.
///
/// The NSView is also already held by the cocoa_dom [`Node`] wrapper;
/// keeping a separate retain here means the tree owns a reference for
/// as long as the node is registered, even if all Node clones drop.
#[derive(Clone)]
pub struct NodeContext {
    pub view: SendWrapper<Retained<NSView>>,
}

/// Owns a Taffy tree plus a slot for the tree's root NodeId. Created
/// once per [`Window`](crate::window); each node registered into the
/// window borrows a clone (Rc-bumped) of this handle so it can address
/// its own slot in the tree later.
///
/// The root NodeId is set by [`register_in_tree`] the first time it
/// runs against an empty tree (i.e. the contentView). Tracked
/// explicitly so the dispatched re-layout pass can find the root
/// without walking — walking via `tree.parent(id)` would panic if the
/// captured id has been removed and its slot reused.
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
/// Idempotent. Uses the node's currently-stored style as the seed
/// and attaches the NSView as the node's Taffy context (used by the
/// measure closure during layout).
///
/// After registration, future style setters push directly into the
/// tree as well as the local style cache.
pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    // Get a Retained<NSView> from the borrowed view ref. Retained's
    // From impl bumps the ObjC retain count.
    let view: Retained<NSView> = node.ns_view().into();
    let context = NodeContext {
        view: SendWrapper::new(view),
    };
    let node_id = tree
        .tree
        .borrow_mut()
        .new_leaf_with_context(layout.style.clone(), context)
        .expect("taffy: new_leaf_with_context failed");
    // First registration in this tree → record the root.
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
        // Capture the parent NodeId before we remove `h.node_id` —
        // after `tree.remove` the parent edge is gone.
        let parent_id = h.tree.tree.borrow().parent(h.node_id);
        let _ = h.tree.tree.borrow_mut().remove(h.node_id);
        if let Some(pid) = parent_id {
            schedule_relayout_for_tree(&h.tree, pid);
        }
    }
}

// ---------------------------------------------------------------------
// Dynamic relayout
// ---------------------------------------------------------------------
//
// When user code mutates the tree at runtime (typically `<For>`/keyed
// iteration adding+removing rows), each mutation mirrors into Taffy
// immediately — but Taffy doesn't *recompute* layout on its own. On
// web Leptos the browser reflows automatically; on AppKit we have to
// call compute_layout ourselves.
//
// Doing it synchronously inside every `attach_child` would be O(N)
// recomputes for an N-row insert. Instead we mark the affected tree
// dirty and dispatch a single recompute on the next main-loop tick.
// Multiple mutations between ticks coalesce into one pass.

thread_local! {
    /// Trees with a layout pass already queued. Keyed by the tree's
    /// Rc identity; cleared at the start of the dispatched callback.
    static PENDING: RefCell<std::collections::HashSet<usize>> =
        RefCell::new(std::collections::HashSet::new());
}

/// Schedule a re-layout of the tree this node belongs to.
///
/// Also marks the node as dirty in Taffy — without this, Taffy's
/// layout cache returns the previously-computed result and the
/// measure callback is never re-invoked. This matters for content
/// changes on leaf controls (label text, button title): the NSView
/// content changed, but Taffy can't see that on its own — we have to
/// tell it explicitly.
pub fn schedule_relayout(node: &Node) {
    let handle = node.layout_slot().borrow().handle.clone();
    if let Some(h) = handle {
        let _ = h.tree.tree.borrow_mut().mark_dirty(h.node_id);
        schedule_relayout_for_tree(&h.tree, h.node_id);
    }
}

fn schedule_relayout_for_tree(tree: &TreeRef, _any_node_id: NodeId) {
    let key = Rc::as_ptr(tree) as usize;
    let just_inserted = PENDING.with_borrow_mut(|p| p.insert(key));
    if !just_inserted {
        return; // already queued
    }
    let tree_weak = SendWrapper::new(Rc::downgrade(tree));
    DispatchQueue::main().exec_async(move || {
        let weak = tree_weak.take();
        let Some(tree) = weak.upgrade() else { return };

        // Clear the pending flag *before* recomputing so any mutation
        // that fires during the recompute can re-enqueue.
        PENDING.with_borrow_mut(|p| {
            p.remove(&(Rc::as_ptr(&tree) as usize));
        });

        // Use the stored root NodeId (set on first registration —
        // never reused or invalidated). Avoids walking, which would
        // panic if any intermediate id was stale.
        let Some(root_id) = *tree.root.borrow() else {
            return;
        };
        let root_view: Retained<NSView> = {
            let tree_ref = tree.tree.borrow();
            let Some(ctx) = tree_ref.get_node_context(root_id) else {
                return;
            };
            let view_ref: &NSView = &**ctx.view;
            view_ref.into()
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
// Tree-edge mirroring (called from cocoa_dom::node insert/remove)
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
    let _ = parent_h
        .tree
        .tree
        .borrow_mut()
        .add_child(parent_h.node_id, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
}

/// Insert a Taffy child at a specific index under `parent`. Same
/// register-if-needed semantics as [`attach_child`].
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
    let _ = parent_h
        .tree
        .tree
        .borrow_mut()
        .insert_child_at_index(parent_h.node_id, index, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
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
    let _ = parent_h
        .tree
        .tree
        .borrow_mut()
        .remove_child(parent_h.node_id, child_id);
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
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

// ---------------------------------------------------------------------
// Layout computation & frame application
// ---------------------------------------------------------------------

/// Compute layout for the subtree rooted at `root`, then walk it and
/// assign frames to each NSView. `available_size` is the size of the
/// region we're laying out into (typically the window's content rect).
///
/// `root` must be registered in a tree. The root's style is forced to
/// fill `available_size` exactly so the layout fills the window
/// content area instead of shrinking to its intrinsic size.
pub fn compute_layout(root: &Node, available_size: NSSize) {
    if layout_debug_enabled() {
        eprintln!(
            "[compute_layout] avail {:.0}x{:.0}",
            available_size.width, available_size.height
        );
    }
    let handle = root.layout_slot().borrow().handle.clone();
    let Some(handle) = handle else {
        if layout_debug_enabled() {
            eprintln!("[compute_layout] BAILED — no handle on root");
        }
        return;
    };

    let w = available_size.width as f32;
    let h = available_size.height as f32;

    let mut tree = handle.tree.tree.borrow_mut();

    // Force the root to fill the available space exactly.
    let mut style = tree
        .style(handle.node_id)
        .cloned()
        .unwrap_or_default();
    style.size = Size {
        width: Dimension::length(w),
        height: Dimension::length(h),
    };
    tree.set_style(handle.node_id, style)
        .expect("taffy: set_style failed");

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    // Use compute_layout_with_measure so leaf controls (NSButton,
    // NSTextField, etc.) get sized to their actual content via
    // `intrinsicContentSize`. Without this, leaves would size to 0
    // (or to a hardcoded placeholder) regardless of their text.
    tree.compute_layout_with_measure(
        handle.node_id,
        avail,
        |known, avail_space, _node_id, ctx, _style| {
            measure_leaf(known, avail_space, ctx)
        },
    )
    .expect("taffy: compute_layout failed");
    apply_layout(&tree, handle.node_id, root.ns_view());
}

/// Measure callback for leaf Taffy nodes. We ask the underlying
/// NSView for its `intrinsicContentSize` — for NSControl-derived
/// views (button, label, text field) this is the size that fits the
/// rendered content (font metrics, button title, etc.). For non-
/// control views (FlippedView containers, Placeholder) the
/// intrinsic is `NSViewNoIntrinsicMetric` (-1) on each axis, which
/// we map to 0.
///
/// `known` carries dimensions that have already been pinned by
/// styling (`size: length(...)`); when present we return them as-is
/// to skip the AppKit call.
fn measure_leaf(
    known: Size<Option<f32>>,
    _avail: Size<AvailableSpace>,
    ctx: Option<&mut NodeContext>,
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

    let view = &**ctx.view;

    // For NSControl (NSButton, NSTextField, etc.), call `sizeToFit`
    // to compute proper bezel-inclusive size, then read the frame.
    // `intrinsicContentSize` alone returns cell content only (no
    // bezel padding) so text gets clipped inside the rendered chrome.
    //
    // For non-control views (FlippedView containers, Placeholder),
    // fall back to intrinsicContentSize (NSViewNoIntrinsicMetric on
    // each axis, mapped to 0).
    let any: &AnyObject = view.as_ref();
    let mut measured: NSSize = if let Some(control) = any.downcast_ref::<NSControl>() {
        let original = view.frame();
        control.sizeToFit();
        let fit = view.frame().size;
        view.setFrame(original);
        fit
    } else {
        view.intrinsicContentSize()
    };

    // Editable text fields: width is NOT content-driven. The user
    // expects the field to be sized by its parent (typical web/UI
    // behaviour: text scrolls horizontally inside a fixed-width
    // box, the box doesn't grow as you type). Returning content-
    // width here would make the field grow with each keystroke.
    // Force width to 0 so the parent (via cross-axis stretch in a
    // Column container, or via flex_grow if the user opts into it)
    // decides the actual width.
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
        // NSViewNoIntrinsicMetric is -1; clamp anything negative to 0
        // so Taffy doesn't get confused by negative sizes.
        if v < 0.0 {
            0.0
        } else {
            v
        }
    }

    Size {
        width: axis(known.width, measured.width),
        height: axis(known.height, measured.height),
    }
}

/// Recursively walk the Taffy tree, copying each node's computed
/// `Layout` into the corresponding NSView's `frame`.
///
/// We iterate over the parent's NSView subviews. AppKit-internal
/// subviews (NSButton's cell, NSTextField's field editor, focus
/// rings) are *not* registered in the Taffy tree, so we filter those
/// out by reading the layout slot off each subview's wrapper. But
/// since we don't have NSView → Node back-mapping, we instead iterate
/// Taffy children and rely on subview index correspondence at each
/// level. This works because `insert_node` mirrors NSView subview
/// order into Taffy children order, and AppKit-internal subviews
/// belong to leaf controls (which never have Taffy children, so we
/// never try to descend into them).
fn apply_layout(
    tree: &TaffyTree<NodeContext>,
    node_id: NodeId,
    view: &NSView,
) {
    let layout: &Layout = tree
        .layout(node_id)
        .expect("taffy: layout missing for node");
    set_frame_from_layout(view, layout);

    let children = tree
        .children(node_id)
        .expect("taffy: children() failed");
    if children.is_empty() {
        // Leaf — don't descend. AppKit-internal subviews are not ours
        // to position.
        return;
    }

    let subviews = view.subviews();
    let subview_count = subviews.count() as usize;
    // Match Taffy children to subviews by position. Taffy children are
    // mirrored from the NSView subview order via insert_node, so the
    // first N subviews correspond 1:1 to the N Taffy children.
    // (Caveat: if AppKit injects a subview under a CONTAINER we own
    // — never observed today — this would skew. Containers we expose
    // as `<view>`/`<stack_view>` are FlippedView, which doesn't add
    // its own subviews.)
    for (i, child_id) in children.iter().enumerate() {
        if i >= subview_count {
            break;
        }
        let sv = subviews.objectAtIndex(i);
        apply_layout(tree, *child_id, &sv);
    }
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
// Convenience setters for common style properties.
// ---------------------------------------------------------------------

pub fn set_width(node: &Node, width_px: f32) {
    update_style(node, |s| {
        s.size.width = Dimension::length(width_px);
    });
}

pub fn set_height(node: &Node, height_px: f32) {
    update_style(node, |s| {
        s.size.height = Dimension::length(height_px);
    });
}

pub fn set_flex_direction(node: &Node, dir: FlexDirection) {
    update_style(node, |s| s.flex_direction = dir);
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
}

pub fn set_gap(node: &Node, gap_px: f32) {
    update_style(node, |s| {
        s.gap = Size {
            width: LengthPercentage::length(gap_px),
            height: LengthPercentage::length(gap_px),
        };
    });
}

pub fn set_justify_content(node: &Node, jc: JustifyContent) {
    update_style(node, |s| s.justify_content = Some(jc));
}

pub fn set_flex_grow(node: &Node, grow: f32) {
    update_style(node, |s| s.flex_grow = grow);
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
}
