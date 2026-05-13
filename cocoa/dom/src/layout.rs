//! Cocoa-side layout adapter.
//!
//! The actual tree storage and Taffy integration live in
//! [`renderer`]; this file plugs cocoa-specific types into it
//! via [`CocoaBackend`]. The wrappers below — `register_in_tree`,
//! `attach_child`, `compute_layout`, the `set_*` setters — read the
//! per-element [`NodeLayout`] slot off a [`Node`] and dispatch into
//! the shared tree.
//!
//! What stays cocoa-specific:
//!
//! - The `CocoaBackend` impl (intrinsic-content-size measurement,
//!   `firstBaselineOffsetFromTop`, `setFrame:`).
//! - The scroll-view second-pass logic (NSScrollView's documentView
//!   gets natural-content sizing; the scroll view itself keeps the
//!   viewport size).
//! - `schedule_relayout` / dispatch (DispatchQueue).
//! - Layer-backed conveniences (`set_background_color`, `set_clip`).

use crate::node::Node;
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSControl, NSTextField, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use send_wrapper::SendWrapper;
use std::{cell::RefCell, rc::Rc, sync::OnceLock};

pub use renderer::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, LengthPercentage, LengthPercentageAuto, NodeId, Position, Size,
    Style, TrackSizingFunction,
};
use renderer::{Layout, LayoutBackend};

// ---------------------------------------------------------------------
// Cocoa backend
// ---------------------------------------------------------------------

/// `LayoutBackend` impl for AppKit. The `View` is a retained pointer
/// to the per-node `NSView`; `NodeMeta` carries the scroll-view flag
/// so the cocoa-side `compute_layout` knows to run a second pass on
/// `<scroll_view>` subtrees.
pub struct CocoaBackend;

#[derive(Clone, Default)]
pub struct CocoaMeta {
    /// True if this node backs an `<scroll_view>` (NSScrollView).
    pub is_scroll_view: bool,
}

impl LayoutBackend for CocoaBackend {
    type View = SendWrapper<Retained<NSView>>;
    type NodeMeta = CocoaMeta;

    fn measure_leaf(
        view: &Self::View,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
    ) -> Size<f32> {
        measure_leaf_size(known, available, view)
    }

    fn first_baseline(view: &Self::View) -> Option<f32> {
        first_baseline_offset(view).map(|b| b as f32)
    }
}

// Aliases so call sites don't have to spell `CocoaBackend` everywhere.
pub type LayoutTree = renderer::LayoutTree<CocoaBackend>;
pub type TreeRef = renderer::TreeRef<CocoaBackend>;
pub type LayoutHandle = renderer::LayoutHandle<CocoaBackend>;
pub type NodeLayout = renderer::NodeLayout<CocoaBackend>;
pub type NodeContext = renderer::NodeContext<CocoaBackend>;

pub fn new_tree() -> TreeRef {
    LayoutTree::new()
}

// ---------------------------------------------------------------------
// Layout debug logging
// ---------------------------------------------------------------------

/// Toggle layout debug output by setting the `COCOA_DOM_LAYOUT_DEBUG`
/// environment variable.
fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("COCOA_DOM_LAYOUT_DEBUG").is_some())
}

// ---------------------------------------------------------------------
// Per-Node helpers (read the `NodeLayout` slot off a `Node`)
// ---------------------------------------------------------------------

/// Register `node` as a leaf in `tree` if not already registered.
pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    let view: Retained<NSView> = node.ns_view().into();
    let view_wrapped = SendWrapper::new(view);
    let node_id =
        tree.new_leaf(layout.style.clone(), view_wrapped, layout.meta.clone());
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

/// Schedule a re-layout of the tree this node belongs to. Marks the
/// node dirty (required for content changes on leaf controls so the
/// measure callback re-runs).
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
            let Some(view) = tree.view(root_id) else { return };
            (*view).clone()
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
///
/// **Applies a frame to the root node itself.** Use
/// [`compute_layout_children`] instead when the root's frame is
/// owned by an outer layout system (e.g. a pane root inside an
/// `NSSplitView` whose Auto-Layout pass already positioned the
/// FlippedView). Calling this on a pane-root would fight the
/// outer system and reset origin to `(0, 0)` every tick.
pub fn compute_layout(root: &Node, available_size: NSSize) {
    compute_layout_inner(root, available_size, /*apply_root_frame=*/ true)
}

/// Like [`compute_layout`] but **skips writing a frame for the
/// root NSView**. The root's frame stays as set by the caller
/// (NSSplitView's Auto-Layout pass, in practice). Taffy still
/// computes the layout using `available_size`, and frames are
/// applied to every descendant.
pub fn compute_layout_children(root: &Node, available_size: NSSize) {
    compute_layout_inner(root, available_size, /*apply_root_frame=*/ false)
}

fn compute_layout_inner(
    root: &Node,
    available_size: NSSize,
    apply_root_frame: bool,
) {
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

    // Cocoa-specific: re-run layout on each `<scroll_view>` subtree
    // with the viewport width pinned and height = MaxContent so the
    // children take their natural sizes. Then restore the scroll
    // view's own final layout to the first-pass viewport size.
    relayout_scroll_views(&handle.tree, handle.node_id);

    if apply_root_frame {
        apply_frames(&handle.tree, handle.node_id);
    } else {
        apply_frames_descendants_only(&handle.tree, handle.node_id);
    }

    // Cocoa-specific: bound each `<scroll_view>` documentView to its
    // children's content extent so NSScrollView shows scroll bars
    // when content overflows.
    fixup_scroll_view_documents(&handle.tree, handle.node_id);

    #[cfg(feature = "debug-overlay")]
    crate::debug_overlay::mark_overlays_dirty();
}

fn is_scroll_view(tree: &TreeRef, id: NodeId) -> bool {
    tree.meta(id).map(|m| m.is_scroll_view).unwrap_or(false)
}

/// Walk the tree from `root`. For each scroll-view, run a second
/// layout pass with viewport width pinned and `MaxContent` height so
/// children take their natural sizes. Restore the scroll view's own
/// final layout to the *first*-pass viewport rect afterward (since
/// the second pass overwrote it with content size).
fn relayout_scroll_views(tree: &TreeRef, root: NodeId) {
    if is_scroll_view(tree, root) {
        let viewport = match tree.layout(root) {
            Some(l) => l,
            None => return,
        };
        let viewport_w = viewport.size.width;

        let saved_style = match tree.style(root) {
            Some(s) => s,
            None => return,
        };
        let mut probe_style = saved_style.clone();
        probe_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::auto(),
        };
        tree.set_style(root, probe_style);
        tree.mark_dirty(root);

        let avail = Size {
            width: AvailableSpace::Definite(viewport_w),
            height: AvailableSpace::MaxContent,
        };
        tree.run_layout_pass(root, avail);

        tree.set_style(root, saved_style);
        tree.mark_dirty(root);
        // Restore the scroll view's own final layout to the first-pass
        // viewport — apply_layout reads `tree.layout(id)` and the
        // second pass left content size in there.
        tree.set_final_layout(root, viewport);
        return;
    }

    // Collect before recursing — `relayout_scroll_views` may call
    // `set_style` on the way back down, which would conflict with
    // an outstanding `Ref` from `children`.
    let kids = tree.children(root).to_vec();
    for child in kids {
        relayout_scroll_views(tree, child);
    }
}

/// For each `<scroll_view>` in the tree, set the NSScrollView's
/// documentView frame to enclose its children's natural extent. This
/// is what makes NSScrollView show scroll bars when content overflows
/// the viewport.
fn fixup_scroll_view_documents(tree: &TreeRef, root: NodeId) {
    if is_scroll_view(tree, root) {
        let Some(view) = tree.view(root) else { return };
        let nsview: &NSView = &**view;
        let any: &AnyObject = nsview.as_ref();
        if let Some(scroll) = any.downcast_ref::<objc2_app_kit::NSScrollView>() {
            if let Some(doc) = scroll.documentView() {
                let viewport = tree.layout(root).unwrap_or_default();
                let mut max_x: f32 = 0.0;
                let mut max_y: f32 = 0.0;
                for &child_id in tree.children(root).iter() {
                    let Some(cl) = tree.layout(child_id) else { continue };
                    max_x = max_x.max(cl.location.x + cl.size.width);
                    max_y = max_y.max(cl.location.y + cl.size.height);
                }
                let doc_w = (max_x as f64).max(viewport.size.width as f64);
                let doc_h = (max_y as f64).max(viewport.size.height as f64);
                doc.setFrame(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(doc_w, doc_h),
                ));
            }
        }
        return;
    }

    for &child in tree.children(root).iter() {
        fixup_scroll_view_documents(tree, child);
    }
}

/// Walk the subtree rooted at `id`, calling `setFrame:` on each
/// node's NSView with its Taffy-computed layout.
fn apply_frames(tree: &TreeRef, id: NodeId) {
    tree.walk_subtree(id, &mut |_id, layout, view| {
        set_frame_from_layout(&view, &layout);
    });
}

/// Same as [`apply_frames`] but skips the root id itself — only
/// descendants get frames assigned. Used by
/// [`compute_layout_children`] for pane-roots whose outer frame is
/// owned by Auto-Layout (e.g. NSSplitView panes).
fn apply_frames_descendants_only(tree: &TreeRef, root_id: NodeId) {
    tree.walk_subtree(root_id, &mut |id, layout, view| {
        if id != root_id {
            set_frame_from_layout(&view, &layout);
        }
    });
}

// ---------------------------------------------------------------------
// Cocoa-specific measure / baseline / setFrame
// ---------------------------------------------------------------------

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
    use renderer::Point;
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
// Generic style setters — lifted to `renderer::setters` and
// generic over `LayoutNodeOps`. The trait impl below wires this
// port's `Node` into that machinery; the `pub use` re-exports keep
// the short paths (`cocoa_dom::layout::set_padding`, etc.) stable.
// ---------------------------------------------------------------------

impl renderer::LayoutNodeOps for Node {
    fn update_style<F: FnOnce(&mut Style)>(&self, f: F) {
        update_style(self, f);
    }
    fn schedule_relayout(&self) {
        schedule_relayout(self);
    }
}

// `LayoutElement` / `UniversalElement` impls let
// `renderer::apply_layout` and `apply_universal` install
// reactive setters against a `cocoa_dom::Element` generically.
impl renderer::LayoutElement for crate::node::Element {
    type Node = Node;
    fn as_node(&self) -> &Self::Node {
        crate::node::Element::as_node(self)
    }
}
impl renderer::UniversalElement for crate::node::Element {
    fn set_alpha(&self, alpha: f64) {
        crate::node::Element::set_alpha(self, alpha)
    }
    fn set_tool_tip(&self, tip: &str) {
        crate::node::Element::set_tool_tip(self, tip)
    }
}

pub use renderer::{
    align_self_to_taffy, apply_layout, apply_universal, dim_to_dimension,
    grid_line_to_placement, set_align_content, set_align_items, set_align_self,
    set_column_gap, set_flex_basis, set_flex_direction, set_flex_grow,
    set_flex_shrink, set_flex_wrap, set_gap, set_grid_auto_columns,
    set_grid_auto_flow, set_grid_auto_rows, set_grid_column_end,
    set_grid_column_start, set_grid_row_end, set_grid_row_start,
    set_grid_template_columns, set_grid_template_rows, set_height,
    set_justify_content, set_justify_items, set_margin, set_max_height,
    set_max_width, set_min_height, set_min_width, set_padding, set_row_gap,
    set_width,
};

// ---------------------------------------------------------------------
// Cocoa-only setters (layer-backed chrome — no analogue in
// renderer-agnostic land).
// ---------------------------------------------------------------------

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

/// Round the view's CALayer corners by `radius` points. 0 disables.
/// Layer-backed conveniences imply `setWantsLayer(true)` like
/// [`set_background_color`].
///
/// Does **not** enable `masksToBounds`: the CALayer's
/// `backgroundColor` already honors `cornerRadius`, so a rounded
/// background shows up without it. Use [`set_clip`] separately when
/// you need child views to clip to the rounded shape (common on
/// container stacks; almost never wanted on buttons, where masking
/// can chew into the rendered title near the corners).
pub fn set_corner_radius(node: &Node, radius: f32) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setCornerRadius(radius as f64);
    }
}

/// Set the CALayer border width in points. `0` disables the border.
/// Border color defaults to opaque black when set the first time;
/// pair with [`set_border_color`] for non-default colors.
pub fn set_border_width(node: &Node, width: f32) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setBorderWidth(width as f64);
    }
}

/// Set the CALayer border color. No effect unless [`set_border_width`]
/// has been called with a width > 0.
pub fn set_border_color(node: &Node, color: crate::Color) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        let ns = color.to_nscolor();
        layer.setBorderColor(Some(&ns.CGColor()));
    }
}
