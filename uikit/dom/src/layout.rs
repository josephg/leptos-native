//! Taffy-based layout engine.
//!
//! Each ios_dom [`Node`] carries a shared "layout slot"
//! ([`NodeLayout`], stored in an `Rc<RefCell<...>>` shared across
//! Node clones). The slot has two pieces:
//!
//!  - the node's *current* style ([`Style`]), mutated by setters and
//!    used as the seed when the node is registered in a tree;
//!  - an `Option<LayoutHandle>` — `Some` once the node has been
//!    registered into a [`TaffyTree`] (i.e. mounted somewhere under a
//!    [`Window`](crate::window)). While `None`, style mutations stay
//!    local; once `Some`, they're also pushed into the tree.
//!
//! Trees themselves are owned by their [`Window`]
//! (`Rc<RefCell<TaffyTree<()>>>`). Each LayoutHandle keeps an Rc to
//! its tree, so late-firing reactive effects can mutate the right
//! tree without consulting any global registry.
//!
//! Unlike the macOS port, we don't need `FlippedView` — UIKit
//! already uses top-left coordinates by default.

use crate::node::Node;
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_ui_kit::{UIControl, UITextField, UIView, UIScrollView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use send_wrapper::SendWrapper;
use std::{cell::RefCell, rc::Rc, sync::OnceLock};

fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("IOS_DOM_LAYOUT_DEBUG").is_some())
}

pub use taffy::{
    AlignItems, AvailableSpace, Dimension, FlexDirection, FlexWrap, JustifyContent,
    LengthPercentage, LengthPercentageAuto, NodeId, Position, Size, Style,
};
use taffy::{Layout, Point, TaffyTree};

/// Per-Taffy-node user data. We attach the underlying UIView so the
/// measure closure can call `UIView::intrinsicContentSize` for leaf
/// controls (UIButton, UILabel, UITextField, etc.).
#[derive(Clone)]
pub struct NodeContext {
    pub view: SendWrapper<Retained<UIView>>,
    /// True if this node backs a `<scroll_view>` (UIScrollView).
    /// Triggers a special second-pass `compute_layout` on this
    /// subtree with `MaxContent` height so children take their
    /// natural sizes — that's what makes the content view grow past
    /// the viewport and gives UIScrollView something to scroll.
    pub is_scroll_view: bool,
}

pub struct LayoutTree {
    pub tree: RefCell<TaffyTree<NodeContext>>,
    pub root: RefCell<Option<NodeId>>,
}

pub type TreeRef = Rc<LayoutTree>;

pub fn new_tree() -> TreeRef {
    Rc::new(LayoutTree {
        tree: RefCell::new(TaffyTree::new()),
        root: RefCell::new(None),
    })
}

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

pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    let view: Retained<UIView> = node.ui_view().into();
    let context = NodeContext {
        view: SendWrapper::new(view),
        is_scroll_view: layout.is_scroll_view,
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

pub fn drop_node(node: &Node) {
    let handle = node.layout_slot().borrow_mut().handle.take();
    if let Some(h) = handle {
        let parent_id = h.tree.tree.borrow().parent(h.node_id);
        let _ = h.tree.tree.borrow_mut().remove(h.node_id);
        if let Some(pid) = parent_id {
            let _ = h.tree.tree.borrow_mut().mark_dirty(pid);
            schedule_relayout_for_tree(&h.tree, pid);
        }
    }
}

// ---------------------------------------------------------------------
// Dynamic relayout
// ---------------------------------------------------------------------

thread_local! {
    static PENDING: RefCell<std::collections::HashSet<usize>> =
        RefCell::new(std::collections::HashSet::new());
}

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
        return;
    }
    let tree_weak = SendWrapper::new(Rc::downgrade(tree));
    DispatchQueue::main().exec_async(move || {
        let weak = tree_weak.take();
        let Some(tree) = weak.upgrade() else { return };

        PENDING.with_borrow_mut(|p| {
            p.remove(&(Rc::as_ptr(&tree) as usize));
        });

        let Some(root_id) = *tree.root.borrow() else {
            return;
        };
        let root_view: Retained<UIView> = {
            let tree_ref = tree.tree.borrow();
            let Some(ctx) = tree_ref.get_node_context(root_id) else {
                return;
            };
            let view_ref: &UIView = &**ctx.view;
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
// Tree-edge mirroring
// ---------------------------------------------------------------------

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
    schedule_relayout_for_tree(&parent_h.tree, parent_h.node_id);
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

pub fn update_style(node: &Node, f: impl FnOnce(&mut Style)) {
    let mut layout = node.layout_slot().borrow_mut();
    f(&mut layout.style);
    if let Some(h) = &layout.handle {
        let _ = h.tree.tree.borrow_mut().set_style(h.node_id, layout.style.clone());
    }
}

pub fn set_style(node: &Node, style: Style) {
    update_style(node, |s| *s = style);
}

// ---------------------------------------------------------------------
// Layout computation & frame application
// ---------------------------------------------------------------------

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
    tree.compute_layout_with_measure(handle.node_id, avail, measure_closure)
        .expect("taffy: compute_layout failed");

    let scroll_view_viewports =
        relayout_scroll_views(&mut tree, handle.node_id);

    apply_layout(
        &tree,
        handle.node_id,
        root.ui_view(),
        &scroll_view_viewports,
    );
}

fn relayout_scroll_views(
    tree: &mut TaffyTree<NodeContext>,
    node_id: NodeId,
) -> std::collections::HashMap<NodeId, Layout> {
    let mut viewports = std::collections::HashMap::new();
    relayout_scroll_views_inner(tree, node_id, &mut viewports);
    viewports
}

fn relayout_scroll_views_inner(
    tree: &mut TaffyTree<NodeContext>,
    node_id: NodeId,
    viewports: &mut std::collections::HashMap<NodeId, Layout>,
) {
    let is_scroll = tree
        .get_node_context(node_id)
        .map(|c| c.is_scroll_view)
        .unwrap_or(false);

    if is_scroll {
        let main_layout = *tree
            .layout(node_id)
            .expect("taffy: layout missing for scroll_view");
        viewports.insert(node_id, main_layout);

        let viewport_w = main_layout.size.width;

        let saved_style = tree
            .style(node_id)
            .expect("taffy: style missing")
            .clone();
        let mut probe_style = saved_style.clone();
        probe_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::auto(),
        };
        let _ = tree.set_style(node_id, probe_style);

        let avail = Size {
            width: AvailableSpace::Definite(viewport_w),
            height: AvailableSpace::MaxContent,
        };
        let _ = tree.mark_dirty(node_id);
        tree.compute_layout_with_measure(node_id, avail, measure_closure)
            .expect("taffy: scroll-view re-layout failed");

        let _ = tree.set_style(node_id, saved_style);
        let _ = tree.mark_dirty(node_id);
        return;
    }

    let children: Vec<NodeId> = tree
        .children(node_id)
        .map(|cs| cs.into_iter().collect())
        .unwrap_or_default();
    for child in children {
        relayout_scroll_views_inner(tree, child, viewports);
    }
}

fn measure_closure(
    known: Size<Option<f32>>,
    avail_space: Size<AvailableSpace>,
    _node_id: NodeId,
    ctx: Option<&mut NodeContext>,
    _style: &Style,
) -> Size<f32> {
    measure_leaf(known, avail_space, ctx)
}

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

    // For UIControl subclasses, call sizeToFit to compute proper
    // content-fitting size, then read the frame. For plain views
    // (containers), fall back to intrinsicContentSize.
    let any: &AnyObject = view.as_ref();
    let mut measured: NSSize = if let Some(control) = any.downcast_ref::<UIControl>() {
        let original = view.frame();
        control.sizeToFit();
        let fit = view.frame().size;
        view.setFrame(original);
        fit
    } else if let Some(label) = any.downcast_ref::<objc2_ui_kit::UILabel>() {
        let original = view.frame();
        label.sizeToFit();
        let fit = view.frame().size;
        view.setFrame(original);
        fit
    } else {
        view.intrinsicContentSize()
    };

    // Editable text fields: width is NOT content-driven (same as macOS).
    // Force width to 0 so the parent decides the actual width.
    if let Some(field) = any.downcast_ref::<UITextField>() {
        if field.isEnabled() {
            measured.width = 0.0;
        }
    }

    fn axis(known: Option<f32>, measured_v: f64) -> f32 {
        if let Some(k) = known {
            return k;
        }
        let v = measured_v as f32;
        // UIViewNoIntrinsicMetric is -1; clamp to 0
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
/// `Layout` into the corresponding UIView's `frame`.
fn apply_layout(
    tree: &TaffyTree<NodeContext>,
    node_id: NodeId,
    view: &UIView,
    scroll_viewports: &std::collections::HashMap<NodeId, Layout>,
) {
    let layout: Layout = if let Some(cached) =
        scroll_viewports.get(&node_id).copied()
    {
        cached
    } else {
        *tree
            .layout(node_id)
            .expect("taffy: layout missing for node")
    };
    set_frame_from_layout(view, &layout);

    let children = tree
        .children(node_id)
        .expect("taffy: children() failed");
    if children.is_empty() {
        return;
    }

    // For `<scroll_view>`, children live inside the content UIView
    // (first subview of the UIScrollView).
    let scroll_content: Option<Retained<UIView>> = {
        let is_ours = tree
            .get_node_context(node_id)
            .map(|c| c.is_scroll_view)
            .unwrap_or(false);
        if is_ours {
            let any: &AnyObject = view.as_ref();
            any.downcast_ref::<UIScrollView>()
                .and_then(|s| {
                    let subs = s.subviews();
                    if subs.count() > 0 {
                        Some(subs.objectAtIndex(0))
                    } else {
                        None
                    }
                })
        } else {
            None
        }
    };
    if let Some(content) = scroll_content.as_ref() {
        // Compute the union of children's allocated rects for
        // UIScrollView's contentSize.
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        for child_id in children.iter() {
            let child_layout = tree
                .layout(*child_id)
                .expect("taffy: child layout missing");
            let right = child_layout.location.x + child_layout.size.width;
            let bottom = child_layout.location.y + child_layout.size.height;
            if right > max_x { max_x = right; }
            if bottom > max_y { max_y = bottom; }
        }
        let content_width = (max_x as f64).max(layout.size.width as f64);
        let content_height = (max_y as f64).max(layout.size.height as f64);
        content.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(content_width, content_height),
        ));

        // Set contentSize on the UIScrollView.
        if let Some(scroll) =
            downcast_scroll(view)
        {
            scroll.setContentSize(NSSize::new(content_width, content_height));
        }
    }

    let subview_source: &UIView =
        scroll_content.as_deref().unwrap_or(view);
    let subviews = subview_source.subviews();
    let subview_count = subviews.count() as usize;
    for (i, child_id) in children.iter().enumerate() {
        if i >= subview_count {
            break;
        }
        let sv = subviews.objectAtIndex(i);
        apply_layout(tree, *child_id, &sv, scroll_viewports);
    }
}

fn set_frame_from_layout(view: &UIView, layout: &Layout) {
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

fn downcast_scroll(view: &UIView) -> Option<&UIScrollView> {
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<UIScrollView>()
}

// ---------------------------------------------------------------------
// Convenience setters
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

/// Force a node's `aspect_ratio` (width / height). Useful for
/// square photo cells (`aspect_ratio = 1.0`).
pub fn set_aspect_ratio(node: &Node, ratio: f32) {
    update_style(node, |s| s.aspect_ratio = Some(ratio));
}

/// Set Taffy's `position` flag. `Position::Absolute` removes the
/// node from the parent's flex layout — it positions itself
/// relative to the parent's content area using `inset_*`. Used
/// for overlay badges (a star in the top-right of a photo cell,
/// a "RAW" chip in the top-left, etc.).
pub fn set_position(node: &Node, position: Position) {
    update_style(node, |s| s.position = position);
}

/// Set the four insets at once. Each value is points; `None`
/// leaves that side as `Auto`. With `Position::Absolute`, an
/// inset of 0 anchors that edge to the parent's content edge.
pub fn set_inset(
    node: &Node,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    left: Option<f32>,
) {
    update_style(node, |s| {
        let to_dim = |v: Option<f32>| match v {
            Some(px) => LengthPercentageAuto::length(px),
            None => LengthPercentageAuto::auto(),
        };
        s.inset = taffy::Rect {
            top: to_dim(top),
            right: to_dim(right),
            bottom: to_dim(bottom),
            left: to_dim(left),
        };
    });
}

/// Per-child override of the parent flex container's `align_items`.
/// `None` means inherit from parent (Taffy's default).
pub fn set_align_self(node: &Node, ai: Option<AlignItems>) {
    update_style(node, |s| s.align_self = ai);
    schedule_relayout(node);
}

/// Convert renderer-common's `Dim` to Taffy's `Dimension`.
pub fn dim_to_dimension(d: renderer::attrs::Dim) -> Dimension {
    use renderer::attrs::Dim;
    match d {
        Dim::Px(v) => Dimension::length(v),
        Dim::Pct(v) => Dimension::percent(v),
        Dim::Auto => Dimension::auto(),
    }
}

/// Convert renderer-common's `AlignSelf` to Taffy's `AlignItems`
/// (Taffy uses one enum for both align-items and align-self).
/// Returns `None` for `AlignSelf::Auto` so the Style stores `None`,
/// meaning "inherit from the parent's `align_items`".
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
