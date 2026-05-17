//! iOS-side layout adapter.
//!
//! Tree storage and Taffy integration live in [`renderer`];
//! this file plugs UIKit-specific types into it via [`IosBackend`].
//! Mirrors `cocoa_dom::layout` one-for-one — the layout problem is
//! the same: lay out flexbox over views whose intrinsic content size
//! comes from native controls (UIButton/UILabel/UITextField). The
//! cocoa-port-style explicit `compute_layout` call after every
//! dirtying mutation is the right fit because UIKit doesn't have a
//! GTK-style measure/allocate protocol — UIView frames are
//! authoritative once set.

use crate::node::Node;
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_ui_kit::{UIControl, UIScrollView, UITextField, UIView};
use send_wrapper::SendWrapper;
use std::{rc::Rc, sync::OnceLock};

pub use renderer::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, Layout, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Rect, Size, Style, TrackSizingFunction,
};
use renderer::LayoutBackend;

// ---------------------------------------------------------------------
// iOS backend
// ---------------------------------------------------------------------

/// `LayoutBackend` impl for UIKit. Same shape as `CocoaBackend` —
/// the `View` is a retained pointer to the per-node UIView; the
/// `NodeMeta` carries the scroll-view flag so the iOS-side
/// `compute_layout` knows to run a second pass on `<scroll_view>`
/// subtrees.
pub struct IosBackend;

#[derive(Clone, Default)]
pub struct IosMeta {
    pub is_scroll_view: bool,
}

impl LayoutBackend for IosBackend {
    type View = SendWrapper<Retained<UIView>>;
    type NodeMeta = IosMeta;

    fn measure_leaf(
        view: &Self::View,
        _meta: &Self::NodeMeta,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
    ) -> Size<f32> {
        measure_leaf_size(known, available, view)
    }

    fn first_baseline(view: &Self::View) -> Option<f32> {
        first_baseline_offset(view).map(|b| b as f32)
    }
}

// Aliases so call sites don't have to spell `IosBackend` everywhere.
pub type LayoutTree = renderer::LayoutTree<IosBackend>;
pub type TreeRef = renderer::TreeRef<IosBackend>;
pub type LayoutHandle = renderer::LayoutHandle<IosBackend>;
pub type NodeLayout = renderer::NodeLayout<IosBackend>;
pub type NodeContext = renderer::NodeContext<IosBackend>;

pub fn new_tree() -> TreeRef {
    LayoutTree::new()
}

// ---------------------------------------------------------------------
// Layout debug logging
// ---------------------------------------------------------------------

fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("IOS_DOM_LAYOUT_DEBUG").is_some())
}

// ---------------------------------------------------------------------
// Per-Node helpers (read the `NodeLayout` slot off a `Node`)
// ---------------------------------------------------------------------

pub fn register_in_tree(node: &Node, tree: &TreeRef) {
    let mut layout = node.layout_slot().borrow_mut();
    if layout.handle.is_some() {
        return;
    }
    let view: Retained<UIView> = node.ui_view().into();
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

pub fn schedule_relayout(node: &Node) {
    let handle = node.layout_slot().borrow().handle.clone();
    if let Some(h) = handle {
        h.tree.mark_dirty(h.node_id);
        schedule_relayout_for_tree(&h.tree, h.node_id);
    }
}

fn schedule_relayout_for_tree(tree: &TreeRef, _any_node_id: NodeId) {
    // Dedup is per-tree via `LayoutTree::relayout_queued`; see the
    // cocoa equivalent for the rationale (avoids a global TLS
    // HashSet and the shutdown-order vulnerability).
    if tree.relayout_queued.replace(true) {
        return;
    }
    let tree_weak = SendWrapper::new(Rc::downgrade(tree));
    DispatchQueue::main().exec_async(move || {
        let weak = tree_weak.take();
        let Some(tree) = weak.upgrade() else { return };

        tree.relayout_queued.set(false);

        let Some(root_id) = *tree.root.borrow() else { return };
        let root_view: Retained<UIView> = {
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

    // iOS-specific: scroll-view second pass (UIScrollView's
    // contentSize-driven layout, mirroring NSScrollView's
    // documentView).
    relayout_scroll_views(&handle.tree, handle.node_id);

    apply_frames(&handle.tree, handle.node_id);
    fixup_scroll_view_contents(&handle.tree, handle.node_id);
}

fn is_scroll_view(tree: &TreeRef, id: NodeId) -> bool {
    tree.meta(id).map(|m| m.is_scroll_view).unwrap_or(false)
}

/// Warn (once per process) when a `<scroll_view>` ends up with a
/// zero-height viewport but has non-empty content. This is the
/// "I put a scroll_view in a parent that doesn't bound its
/// height" footgun — see book / `docs/book/src/layout/scroll.md`.
///
/// Per the failure-mode hierarchy in CLAUDE.md, we choose warn +
/// graceful degrade (the user gets a blank scroll view) over a
/// panic — runtime layout state is too dependent on transient
/// inputs (window size, parent flex_grow, dynamic content) to
/// safely panic.
fn warn_if_scroll_view_unsized(
    tree: &TreeRef,
    root: NodeId,
    viewport: &taffy::Layout,
) {
    use std::sync::Once;
    static WARNED: Once = Once::new();

    if viewport.size.height < 0.5 && !tree.children(root).is_empty() {
        WARNED.call_once(|| {
            eprintln!(
                "[ios_dom] a <scroll_view> has zero-height viewport \
                 but non-empty children — it will render blank. \
                 The most common cause is the scroll_view's parent \
                 not having a bounded main-axis size. Fix by \
                 setting `flex_grow=1.0` on the scroll_view (and on \
                 its parent if that parent is itself unbounded), \
                 or by giving it an explicit `height`. See \
                 docs/book/src/layout/scroll.md. (This warning \
                 prints once per process.)"
            );
        });
    }
}

fn relayout_scroll_views(tree: &TreeRef, root: NodeId) {
    if is_scroll_view(tree, root) {
        let viewport = match tree.layout(root) {
            Some(l) => l,
            None => return,
        };
        warn_if_scroll_view_unsized(tree, root, &viewport);
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
        // Restore the scroll view's own final layout to the
        // first-pass viewport — apply_layout reads `tree.layout(id)`
        // and the second pass left content size in there.
        tree.set_final_layout(root, viewport);
        return;
    }

    let kids = tree.children(root).to_vec();
    for child in kids {
        relayout_scroll_views(tree, child);
    }
}

fn apply_frames(tree: &TreeRef, id: NodeId) {
    tree.walk_subtree(id, &mut |_id, layout, view| {
        set_frame_from_layout(&view, &layout);
    });
}

/// Bound each `<scroll_view>` content view to its children's natural
/// extent, and update the UIScrollView's `contentSize` so it scrolls.
fn fixup_scroll_view_contents(tree: &TreeRef, root: NodeId) {
    if is_scroll_view(tree, root) {
        let Some(view) = tree.view(root) else { return };
        let uiview: &UIView = &**view;
        let any: &AnyObject = uiview.as_ref();
        if let Some(scroll) = any.downcast_ref::<UIScrollView>() {
            let viewport = tree.layout(root).unwrap_or_default();
            let mut max_x: f32 = 0.0;
            let mut max_y: f32 = 0.0;
            for &child_id in tree.children(root).iter() {
                let Some(cl) = tree.layout(child_id) else { continue };
                max_x = max_x.max(cl.location.x + cl.size.width);
                max_y = max_y.max(cl.location.y + cl.size.height);
            }
            let cw = (max_x as f64).max(viewport.size.width as f64);
            let ch = (max_y as f64).max(viewport.size.height as f64);
            // The "content view" we install for `<scroll_view>` is
            // the first subview of the UIScrollView (analogous to
            // NSScrollView's documentView).
            let subs = scroll.subviews();
            if subs.count() > 0 {
                let content = subs.objectAtIndex(0);
                content.setFrame(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(cw, ch),
                ));
            }
            scroll.setContentSize(NSSize::new(cw, ch));
        }
        return;
    }

    for &child in tree.children(root).iter() {
        fixup_scroll_view_contents(tree, child);
    }
}

// ---------------------------------------------------------------------
// iOS-specific measure / baseline / setFrame
// ---------------------------------------------------------------------

fn measure_leaf_size(
    known: Size<Option<f32>>,
    _avail: Size<AvailableSpace>,
    view: &UIView,
) -> Size<f32> {
    if let (Some(w), Some(h)) = (known.width, known.height) {
        return Size { width: w, height: h };
    }

    let any: &AnyObject = view.as_ref();
    // For UIControl subclasses, call `sizeToFit` then read the frame.
    // For UILabel (not a UIControl on iOS) sizeToFit is also the
    // right entry point.
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

    // Editable text fields: width is NOT content-driven (otherwise
    // the field grows with each keystroke). Mirror cocoa's trick.
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
        // UIViewNoIntrinsicMetric is -1; clamp to 0.
        if v < 0.0 { 0.0 } else { v }
    }

    Size {
        width: axis(known.width, measured.width),
        height: axis(known.height, measured.height),
    }
}

/// First-baseline offset from the top of `view`'s frame.
///
/// UIView doesn't expose `firstBaselineOffsetFromTop` (that's
/// macOS-only — UIKit has `firstBaselineAnchor`, an AutoLayout
/// primitive that's awkward to resolve outside the AutoLayout
/// engine). We compute it from the leaf's font metrics + alignment
/// rect:
///
/// - **Tall bezel** (UIButton-style — alignment rect taller than
///   one line): cap height is centered in the alignment rect.
///   `baseline = align_top + align_h/2 + cap_height/2`.
/// - **Tight box** (UILabel/UITextField hugging its line): baseline
///   is at `align_top + ascender`.
///
/// Returns `None` for views without a queryable font (containers,
/// images, etc.).
pub fn first_baseline_offset(view: &UIView) -> Option<f64> {
    use objc2_ui_kit::{UIButton, UIFont, UILabel};
    let any: &AnyObject = view.as_ref();

    let font: Option<Retained<UIFont>> = if let Some(label) =
        any.downcast_ref::<UILabel>()
    {
        label.font()
    } else if let Some(button) = any.downcast_ref::<UIButton>() {
        // Read the font through `titleLabel` (a UILabel) — UIButton's
        // `-font` is deprecated in favour of attributed-string-based
        // titles, but the title still renders through this label.
        button.titleLabel().and_then(|l| l.font())
    } else if let Some(field) = any.downcast_ref::<UITextField>() {
        field.font()
    } else {
        None
    };
    let font = font?;

    let frame_h = view.frame().size.height;
    let insets = view.alignmentRectInsets();
    let align_top = insets.top as f64;
    let align_bottom = insets.bottom as f64;
    let align_h = (frame_h - align_top - align_bottom).max(0.0);
    let ascender = unsafe { font.ascender() } as f64;
    let descender = unsafe { font.descender() } as f64; // negative
    let cap_height = unsafe { font.capHeight() } as f64;
    let line_h = ascender - descender;

    // 2px tolerance — alignment rects "tight" to the line can differ
    // by sub-pixel rounding.
    let baseline = if align_h > line_h + 2.0 {
        align_top + align_h * 0.5 + cap_height * 0.5
    } else {
        align_top + ascender
    };
    Some(baseline)
}

fn set_frame_from_layout(view: &UIView, layout: &Layout) {
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
// Generic style setters — lifted to `renderer::setters`. See the
// cocoa port's equivalent block for the design rationale.
// ---------------------------------------------------------------------

impl renderer::LayoutNodeOps for Node {
    fn update_style<F: FnOnce(&mut Style)>(&self, f: F) {
        update_style(self, f);
    }
    fn schedule_relayout(&self) {
        schedule_relayout(self);
    }
    fn with_style<R, F: FnOnce(&Style) -> R>(&self, f: F) -> R {
        let slot = self.layout_slot().borrow();
        f(&slot.style)
    }
}

// iOS Element impls — `set_tool_tip` uses the default no-op
// (UIView has no hover tooltips).
impl renderer::LayoutElement for crate::node::Element {
    type Node = Node;
    fn as_node(&self) -> &Self::Node {
        crate::node::Element::as_node(self)
    }
    fn set_view_hidden(&self, hidden: bool) {
        crate::node::Element::set_bool_attribute(
            self,
            crate::node::BoolAttr::Hidden,
            hidden,
        );
    }
    // `set_clip`: iOS hasn't wired UIView::clipsToBounds yet, so
    // `overflow=Hidden` is layout-only on this port (Taffy
    // auto-min-size becomes 0, no visual clip). Override when
    // clip support lands.
}
impl renderer::UniversalElement for crate::node::Element {
    fn set_alpha(&self, alpha: f64) {
        crate::node::Element::set_alpha(self, alpha)
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
    set_max_width, set_min_height, set_min_width, set_overflow, set_padding,
    set_row_gap, set_width,
};

// ---------------------------------------------------------------------
// iOS-only setters — Taffy fields the cocoa port doesn't currently
// expose (aspect_ratio, position, inset), used by the UIView builders
// for square photo cells and overlay badges.
// ---------------------------------------------------------------------

/// Force a node's `aspect_ratio` (width / height). Useful for
/// square photo cells (`aspect_ratio = 1.0`).
pub fn set_aspect_ratio(node: &Node, ratio: f32) {
    update_style(node, |s| s.aspect_ratio = Some(ratio));
    schedule_relayout(node);
}

/// Set Taffy's `position` flag. `Position::Absolute` removes the
/// node from the parent's flex layout — it positions itself
/// relative to the parent's content area using `inset_*`. Used for
/// overlay badges.
pub fn set_position(node: &Node, position: Position) {
    update_style(node, |s| s.position = position);
    schedule_relayout(node);
}

/// Set the four insets at once. Each value is points; `None`
/// leaves that side as `Auto`. With `Position::Absolute`, an inset
/// of 0 anchors that edge to the parent's content edge.
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
        s.inset = Rect {
            top: to_dim(top),
            right: to_dim(right),
            bottom: to_dim(bottom),
            left: to_dim(left),
        };
    });
    schedule_relayout(node);
}
