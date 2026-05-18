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
use std::{rc::Rc, sync::OnceLock};

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
/// and the documentView-wrapper redirect used by `<scroll_view>`.
pub struct CocoaBackend;

#[derive(Clone, Default)]
pub struct CocoaMeta {
    /// True if this node backs an `<scroll_view>` (NSScrollView).
    pub is_scroll_view: bool,
    /// Which axis (or axes) the scroll view scrolls on. Drives the
    /// documentView wrapper's Taffy style: Vertical lets the
    /// wrapper grow vertically and locks its width to the viewport;
    /// Horizontal flips the axis; Both lets it grow on both axes.
    /// Only meaningful when `is_scroll_view`. Default Vertical.
    pub scroll_axis: ScrollAxis,
    /// For `<scroll_view>`: the NodeId of an intermediate Taffy node
    /// backed by the NSScrollView's documentView. Children added to
    /// the scroll view at the Node/AppKit layer are attached to this
    /// wrapper at the Taffy layer instead of the scroll view itself.
    ///
    /// The wrapper has `flex_shrink: 0` so Taffy sizes it to its
    /// children's natural extent (not the scroll view's viewport),
    /// giving the documentView its scrollable content size in a
    /// single layout pass. `apply_frames` then writes that size to
    /// the documentView's `setFrame:` naturally — no second pass
    /// and no post-hoc fixup needed.
    pub child_taffy_parent: Option<NodeId>,
    /// `<text_field intrinsic_width=FromContent>` opt-in: read the
    /// NSTextField's natural content width instead of forcing
    /// width=0 in the measure pass. Without this the editable text
    /// field grows with each keystroke as `intrinsicContentSize`
    /// tracks the live string. Lives on meta (not a sidetable)
    /// because Taffy hands it to `measure_leaf` alongside the view.
    pub intrinsic_width_from_content: bool,
}

/// Which axis (or axes) a `<scroll_view>` scrolls on. Picked at
/// `<scroll_view axis=...>` build time; not reactive (it sets the
/// documentView wrapper's Taffy style at registration time, plus
/// sensible scroller-visibility defaults).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScrollAxis {
    /// Content grows downward; viewport width locks to the scroll
    /// view's width. Vertical wheel/trackpad input scrolls; horizontal
    /// is a no-op. This is the default and matches the most common
    /// macOS scroll-view pattern.
    #[default]
    Vertical,
    /// Content grows rightward; viewport height locks to the scroll
    /// view's height. Horizontal input scrolls.
    Horizontal,
    /// Content grows on both axes; both directions scroll. The
    /// documentView is sized to the natural extent of its content.
    Both,
}

impl LayoutBackend for CocoaBackend {
    type View = SendWrapper<Retained<NSView>>;
    type NodeMeta = CocoaMeta;

    fn measure_leaf(
        view: &Self::View,
        meta: &Self::NodeMeta,
        known: Size<Option<f32>>,
        available: Size<AvailableSpace>,
    ) -> Size<f32> {
        measure_leaf_size(known, available, view, meta)
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
///
/// `<scroll_view>` nodes additionally allocate a second Taffy leaf
/// backed by the NSScrollView's documentView (the "child taffy
/// parent"). The wrapper has `flex_shrink: 0` and `flex_direction:
/// Column`, so it sizes to its content's natural extent and
/// overflows the scroll view's clipped viewport — which is exactly
/// what we want for the documentView's frame. Subsequent
/// `attach_child` calls redirect into this wrapper, so the user's
/// children are laid out inside it.
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

    if layout.meta.is_scroll_view {
        if let Some(doc) = scroll_view_document(node.ns_view()) {
            // Wrapper style picks the scroll axis. The principle:
            // on the scroll axis, the wrapper grows to natural
            // content size (flex_shrink:0 keeps it from being
            // squashed to fit the viewport); on a non-scroll axis,
            // the wrapper's size is bounded to the viewport via the
            // default align_items: Stretch on the parent flex,
            // which means children that would overflow that axis
            // simply paint outside the wrapper (NSScrollView's
            // clipView still clips them visually).
            //
            // For Both, neither axis stretches — the wrapper takes
            // its natural content size on both directions.
            // The wrapper uses `position: Absolute` rather than
            // participating in the scroll_view's flex flow. Two
            // reasons:
            //
            // 1. **Intrinsic-sizing isolation.** An absolutely-
            //    positioned child does NOT contribute to its parent's
            //    max-content / min-content. With a flex wrapper,
            //    Taffy's max-content for the scroll_view would
            //    include the wrapper's natural content size, and that
            //    propagates UP to every ancestor's intrinsic size
            //    computation — inflating the entire layout to fit
            //    the scrollable content. `overflow: Hidden` on the
            //    scroll_view stops *content_size* propagation in the
            //    layout pass but doesn't suppress max-content
            //    propagation, so a flex wrapper still poisons
            //    ancestor sizing.
            //
            // 2. **Bounded by viewport per axis.** With
            //    `inset.top: 0, inset.left: 0` plus `size: auto`,
            //    the wrapper sits at the scroll_view's top-left and
            //    sizes to its content on whichever axes scroll. For
            //    axes that *don't* scroll, we additionally set the
            //    opposite inset to 0 so the wrapper's cross-axis
            //    size matches the viewport (e.g. for Vertical,
            //    inset.right: 0 → wrapper.width = scroll_view.width).
            //
            // Vertical scroll → expand horizontally to viewport, grow
            // vertically with content.
            // Horizontal scroll → expand vertically to viewport, grow
            // horizontally with content.
            // Both → corner-pin only; grow on both axes.
            use taffy::{LengthPercentageAuto, Position};
            let zero = LengthPercentageAuto::length(0.0);
            let mut wrapper_style = Style::default();
            wrapper_style.position = Position::Absolute;
            wrapper_style.inset.top = zero;
            wrapper_style.inset.left = zero;
            match layout.meta.scroll_axis {
                ScrollAxis::Vertical => {
                    wrapper_style.flex_direction = FlexDirection::Column;
                    // Lock cross-axis (horizontal) to viewport.
                    wrapper_style.inset.right = zero;
                }
                ScrollAxis::Horizontal => {
                    wrapper_style.flex_direction = FlexDirection::Row;
                    // Lock cross-axis (vertical) to viewport.
                    wrapper_style.inset.bottom = zero;
                }
                ScrollAxis::Both => {
                    wrapper_style.flex_direction = FlexDirection::Column;
                    // Neither cross-axis pinned; wrapper takes
                    // content size on both axes.
                }
            }
            let wrapper_id = tree.new_leaf(
                wrapper_style,
                SendWrapper::new(doc),
                CocoaMeta::default(),
            );
            tree.add_child(node_id, wrapper_id);
            layout.meta.child_taffy_parent = Some(wrapper_id);
            tree.set_meta(node_id, layout.meta.clone());
        }
    }

    layout.handle = Some(LayoutHandle {
        tree: tree.clone(),
        node_id,
    });
}

fn scroll_view_document(view: &NSView) -> Option<Retained<NSView>> {
    let any: &AnyObject = view.as_ref();
    any.downcast_ref::<objc2_app_kit::NSScrollView>()
        .and_then(|s| s.documentView())
}

/// Drop the node and unregister it. No-op if never registered.
pub fn drop_node(node: &Node) {
    let (handle, wrapper_id) = {
        let mut slot = node.layout_slot().borrow_mut();
        let wrapper = slot.meta.child_taffy_parent.take();
        (slot.handle.take(), wrapper)
    };
    if let Some(h) = handle {
        let parent_id = h.tree.parent(h.node_id);
        if let Some(w) = wrapper_id {
            h.tree.remove(w);
        }
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
    // Phase 2: snapshot the in-flight animation BEFORE the
    // dedup check. The deferred compute_layout runs after
    // CURRENT has been cleared, so we capture here and read at
    // compute time. Setting unconditionally (even if the dedup
    // returns early) ensures an animated setter that follows a
    // non-animated one still gets its animation honoured.
    #[cfg(feature = "animation")]
    crate::animation::capture_for_layout();

    // Dedup: each tree carries its own "is a pass queued?" flag.
    // Cheap, no global state, no shutdown-order issue.
    if tree.relayout_queued.replace(true) {
        return;
    }
    let tree_weak = SendWrapper::new(Rc::downgrade(tree));
    DispatchQueue::main().exec_async(move || {
        let weak = tree_weak.take();
        let Some(tree) = weak.upgrade() else { return };

        tree.relayout_queued.set(false);

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

/// Returns `(tree, taffy_parent_id)` for use when attaching children
/// in the Taffy tree. For `<scroll_view>` this redirects to the
/// documentView wrapper (`meta.child_taffy_parent`); for everything
/// else it's the node's own Taffy id.
fn taffy_child_parent(parent: &Node) -> Option<(TreeRef, NodeId)> {
    let slot = parent.layout_slot().borrow();
    let h = slot.handle.as_ref()?;
    let id = slot.meta.child_taffy_parent.unwrap_or(h.node_id);
    Some((h.tree.clone(), id))
}

pub fn attach_child(parent: &Node, child: &Node) {
    let Some((tree, parent_id)) = taffy_child_parent(parent) else { return };
    register_in_tree(child, &tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    tree.add_child(parent_id, child_id);
    schedule_relayout_for_tree(&tree, parent_id);
}

pub fn insert_child_at(parent: &Node, child: &Node, index: usize) {
    let Some((tree, parent_id)) = taffy_child_parent(parent) else { return };
    register_in_tree(child, &tree);
    let child_id = child
        .layout_slot()
        .borrow()
        .handle
        .as_ref()
        .expect("just registered")
        .node_id;
    tree.insert_child_at_index(parent_id, index, child_id);
    schedule_relayout_for_tree(&tree, parent_id);
}

pub fn detach_child(parent: &Node, child: &Node) {
    let Some((tree, parent_id)) = taffy_child_parent(parent) else { return };
    let child_id = match child.layout_slot().borrow().handle.as_ref() {
        Some(h) => h.node_id,
        None => return,
    };
    tree.remove_child(parent_id, child_id);
    schedule_relayout_for_tree(&tree, parent_id);
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

    warn_zero_height_scroll_views(&handle.tree, handle.node_id);

    if apply_root_frame {
        apply_frames(&handle.tree, handle.node_id);
    } else {
        apply_frames_descendants_only(&handle.tree, handle.node_id);
    }

    #[cfg(feature = "debug-overlay")]
    crate::debug_overlay::mark_overlays_dirty();
}

/// Warn (once per process) when a `<scroll_view>` ends up with a
/// zero-height viewport but has non-empty content. The most
/// common cause is a parent that doesn't bound its main-axis
/// size — see `docs/book/src/layout/scroll.md`.
///
/// Per the failure-mode hierarchy in CLAUDE.md, this is
/// warn-and-degrade rather than panic: runtime layout state
/// depends on transient inputs (window size, parent flex_grow,
/// dynamic content) and a panic here could crash a user's
/// production app for a non-showstopper. A blank scroll view is
/// undesirable but recoverable.
fn warn_zero_height_scroll_views(tree: &TreeRef, root: NodeId) {
    use std::sync::Once;
    static WARNED: Once = Once::new();

    fn visit(tree: &TreeRef, id: NodeId, warned: &Once) {
        let is_sv = tree
            .meta(id)
            .map(|m| m.is_scroll_view)
            .unwrap_or(false);
        if is_sv {
            if let Some(layout) = tree.layout(id) {
                let has_children = !tree.children(id).is_empty();
                if has_children && layout.size.height < 0.5 {
                    warned.call_once(|| {
                        eprintln!(
                            "[cocoa_dom] a <scroll_view> has \
                             zero-height viewport but non-empty \
                             children — it will render blank. The \
                             most common cause is the scroll_view's \
                             parent not having a bounded main-axis \
                             size. Fix by setting `flex_grow=1.0` \
                             on the scroll_view (and on its parent \
                             if that parent is itself unbounded), \
                             or by giving it an explicit `height`. \
                             See docs/book/src/layout/scroll.md. \
                             (This warning prints once per \
                             process.)"
                        );
                    });
                }
            }
        }
        let kids = tree.children(id).to_vec();
        for k in kids {
            visit(tree, k, warned);
        }
    }
    visit(tree, root, &WARNED);
}

/// Walk the subtree rooted at `id`, calling `setFrame:` on each
/// node's NSView with its Taffy-computed layout.
fn apply_frames(tree: &TreeRef, id: NodeId) {
    #[cfg(feature = "animation")]
    let pending = crate::animation::take_pending_layout_animation();
    #[cfg(not(feature = "animation"))]
    let pending: Option<()> = None;
    tree.walk_subtree(id, &mut |_id, layout, view| {
        set_frame_from_layout(&view, &layout, pending);
    });
}

/// Same as [`apply_frames`] but skips the root id itself — only
/// descendants get frames assigned. Used by
/// [`compute_layout_children`] for pane-roots whose outer frame is
/// owned by Auto-Layout (e.g. NSSplitView panes).
fn apply_frames_descendants_only(tree: &TreeRef, root_id: NodeId) {
    #[cfg(feature = "animation")]
    let pending = crate::animation::take_pending_layout_animation();
    #[cfg(not(feature = "animation"))]
    let pending: Option<()> = None;
    tree.walk_subtree(root_id, &mut |id, layout, view| {
        if id != root_id {
            set_frame_from_layout(&view, &layout, pending);
        }
    });
}

// ---------------------------------------------------------------------
// Cocoa-specific measure / baseline / setFrame
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Per-element flag: `intrinsic_width = FromContent` on <text_field>.
// Lives on the node's `CocoaMeta` and is passed to `measure_leaf`
// alongside the view — no sidetable, no teardown plumbing.
// ---------------------------------------------------------------------

/// Mark this node's NSTextField as "use natural content width."
/// Default for editable NSTextField is width=0 in the measure pass;
/// this flag flips that to read `intrinsicContentSize` like a label.
pub(crate) fn mark_intrinsic_width_from_content(node: &Node, on: bool) {
    let mut slot = node.layout_slot().borrow_mut();
    slot.meta.intrinsic_width_from_content = on;
    if let Some(h) = &slot.handle {
        h.tree.set_meta(h.node_id, slot.meta.clone());
        h.tree.mark_dirty(h.node_id);
    }
}

fn measure_leaf_size(
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
    view: &NSView,
    meta: &CocoaMeta,
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

    // Editable text fields: width is NOT content-driven by default
    // (otherwise the field grows with each keystroke). Force width
    // to 0 so the parent decides via cross-axis stretch / flex_grow.
    //
    // Opt-out: callers can mark a field as "keep content width" via
    // `Element::set_intrinsic_width_from_content(true)`. See the
    // `intrinsic_width` builder method on `<text_field>`.
    if let Some(field) = any.downcast_ref::<NSTextField>() {
        if field.isEditable() && !meta.intrinsic_width_from_content {
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

fn set_frame_from_layout(
    view: &NSView,
    layout: &Layout,
    #[cfg(feature = "animation")] pending_anim: Option<crate::animation::Animation>,
    #[cfg(not(feature = "animation"))] _pending_anim: Option<()>,
) {
    use renderer::Point;
    let Point { x, y } = layout.location;
    let Size { width, height } = layout.size;
    if layout_debug_enabled() {
        eprintln!(
            "  [frame] {:p} <- ({:.0},{:.0}) {:.0}x{:.0}",
            view as *const _, x, y, width, height
        );
    }

    // Phase 2: capture layer's pre-mutation position/bounds so
    // we can animate over the captured-to-new delta. Layer-backed
    // views only; we don't force `wantsLayer = true` here because
    // that would silently change rendering for many NSView
    // subclasses. Document: layout animation requires the
    // animated view (or an ancestor) to already be layer-backed
    // — typically via `background_color`, `corner_radius`, or an
    // explicit `setWantsLayer(true)`.
    //
    // From-values read the PRESENTATION layer so that interrupting
    // a running frame animation continues smoothly from the visible
    // frame instead of snapping to the model.
    #[cfg(feature = "animation")]
    let snapshot = pending_anim.and_then(|anim| {
        view.layer().map(|layer| {
            let visible_pos = crate::animation::presentation_or_model(
                &layer, |l| l.position()
            );
            let visible_bounds = crate::animation::presentation_or_model(
                &layer, |l| l.bounds()
            );
            (anim, layer.clone(), visible_pos, visible_bounds)
        })
    });

    view.setFrame(NSRect::new(
        NSPoint::new(x as f64, y as f64),
        NSSize::new(width as f64, height as f64),
    ));

    #[cfg(feature = "animation")]
    if let Some((anim, layer, old_position, old_bounds)) = snapshot {
        let new_position = layer.position();
        let new_bounds = layer.bounds();
        // Skip if no actual change — avoids a spurious
        // animation queued on every relayout tick.
        let pos_moved =
            (old_position.x - new_position.x).abs() > f64::EPSILON
                || (old_position.y - new_position.y).abs() > f64::EPSILON;
        let bounds_changed = (old_bounds.size.width - new_bounds.size.width)
            .abs() > f64::EPSILON
            || (old_bounds.size.height - new_bounds.size.height).abs()
                > f64::EPSILON;
        if pos_moved || bounds_changed {
            crate::animation::animate_frame(
                &layer,
                old_position,
                old_bounds,
                new_position,
                new_bounds,
                anim,
            );
        }
    }
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
    fn with_style<R, F: FnOnce(&Style) -> R>(&self, f: F) -> R {
        let slot = self.layout_slot().borrow();
        f(&slot.style)
    }
}

// `LayoutElement` / `UniversalElement` / `DecorationElement` impls let
// `renderer::apply_layout` / `apply_universal` / `apply_decoration`
// install reactive setters against a `cocoa_dom::Element` generically.
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
    fn set_clip(&self, clip: bool) {
        set_clip(self.as_node(), clip);
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
impl renderer::DecorationElement<crate::Color> for crate::node::Element {
    fn set_background_color(&self, color: crate::Color) {
        set_background_color(self.as_node(), color);
    }
    fn set_corner_radius(&self, radius: f32) {
        set_corner_radius(self.as_node(), radius);
    }
    fn set_border_width(&self, width: f32) {
        set_border_width(self.as_node(), width);
    }
    fn set_border_color(&self, color: crate::Color) {
        set_border_color(self.as_node(), color);
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
// Cocoa-only setters (layer-backed chrome — no analogue in
// renderer-agnostic land).
// ---------------------------------------------------------------------

pub fn set_background_color(node: &Node, color: crate::Color) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        let ns_color = color.to_nscolor();
        let new_cg = ns_color.CGColor();
        #[cfg(feature = "animation")]
        let old_cg = crate::animation::presentation_or_model(
            &layer, |l| l.backgroundColor()
        );
        layer.setBackgroundColor(Some(&new_cg));
        #[cfg(feature = "animation")]
        crate::animation::animate_color(
            &layer,
            "backgroundColor",
            old_cg.as_deref(),
            Some(&new_cg),
        );
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
        #[cfg(feature = "animation")]
        let old = crate::animation::presentation_or_model(
            &layer, |l| l.cornerRadius()
        );
        layer.setCornerRadius(radius as f64);
        #[cfg(feature = "animation")]
        crate::animation::animate_float(
            &layer,
            "cornerRadius",
            old,
            radius as f64,
        );
    }
}

/// Set the CALayer border width in points. `0` disables the border.
/// Border color defaults to opaque black when set the first time;
/// pair with [`set_border_color`] for non-default colors.
pub fn set_border_width(node: &Node, width: f32) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        #[cfg(feature = "animation")]
        let old = crate::animation::presentation_or_model(
            &layer, |l| l.borderWidth()
        );
        layer.setBorderWidth(width as f64);
        #[cfg(feature = "animation")]
        crate::animation::animate_float(
            &layer,
            "borderWidth",
            old,
            width as f64,
        );
    }
}

/// Apply a 2D scale transform to the view's CALayer. `(1.0, 1.0)`
/// Apply a 2D translation to the view's CALayer (in points). `(0, 0)`
/// is identity. Independent of `setFrame:` — moves the rendered
/// layer without touching Taffy's layout.
///
/// When called inside `with_animation(...)`, animates
/// `transform.translation.{x,y}` from their prior values. Useful
/// for slide-in / slide-out effects without disturbing layout.
#[cfg(feature = "animation")]
pub fn set_translation(node: &Node, tx: f64, ty: f64) {
    use objc2_quartz_core::CATransform3D;
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        let model = layer.transform();
        // From-value reads the PRESENTATION layer so that
        // interrupting a running animation continues smoothly
        // from the visible position instead of snapping to the
        // model.
        let visible = crate::animation::presentation_or_model(
            &layer, |l| l.transform()
        );
        let old_tx = visible.m41;
        let old_ty = visible.m42;
        // Preserve any existing scale (m11/m22) on the model. If
        // the layer has no scale set yet (m11/m22 == 0 → identity
        // is uninitialised), treat as 1.0.
        let sx = if model.m11 == 0.0 { 1.0 } else { model.m11 };
        let sy = if model.m22 == 0.0 { 1.0 } else { model.m22 };
        let mut new_t = CATransform3D::new_scale(sx, sy, 1.0);
        new_t.m41 = tx;
        new_t.m42 = ty;
        layer.setTransform(new_t);
        if let Some(anim) = crate::animation::current_animation() {
            let from_x = objc2_foundation::NSNumber::new_f64(old_tx);
            let to_x = objc2_foundation::NSNumber::new_f64(tx);
            crate::animation::apply_property_animation(
                &layer,
                "transform.translation.x",
                Some(from_x.as_ref()),
                Some(to_x.as_ref()),
                anim,
            );
            let from_y = objc2_foundation::NSNumber::new_f64(old_ty);
            let to_y = objc2_foundation::NSNumber::new_f64(ty);
            crate::animation::apply_property_animation(
                &layer,
                "transform.translation.y",
                Some(from_y.as_ref()),
                Some(to_y.as_ref()),
                anim,
            );
        }
    }
}

/// Apply a 2D scale transform to the view's CALayer. `(1.0, 1.0)`
/// is identity (no transform installed). Negative values flip,
/// `0.0` collapses. The scale is anchored at the layer's current
/// `anchorPoint` (default = centre).
///
/// When called inside `with_animation(...)`, animates
/// `transform.scale.x` and `transform.scale.y` from their prior
/// values. Available only with the `animation` Cargo feature.
#[cfg(feature = "animation")]
pub fn set_scale(node: &Node, sx: f64, sy: f64) {
    use objc2_quartz_core::CATransform3D;
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        // From-value reads the PRESENTATION layer so an
        // interrupted prior animation continues smoothly from the
        // visible scale instead of snapping to the model.
        let visible = crate::animation::presentation_or_model(
            &layer, |l| l.transform()
        );
        let old_sx = visible.m11; // x-scale stored on m11
        let old_sy = visible.m22; // y-scale stored on m22
        // Preserve translation components — otherwise a co-resident
        // set_translation would have its m41/m42 clobbered when
        // set_scale rewrote the matrix.
        let model = layer.transform();
        let mut new_t = CATransform3D::new_scale(sx, sy, 1.0);
        new_t.m41 = model.m41;
        new_t.m42 = model.m42;
        new_t.m43 = model.m43;
        layer.setTransform(new_t);
        if let Some(anim) = crate::animation::current_animation() {
            let from_x = objc2_foundation::NSNumber::new_f64(old_sx);
            let to_x = objc2_foundation::NSNumber::new_f64(sx);
            crate::animation::apply_property_animation(
                &layer,
                "transform.scale.x",
                Some(from_x.as_ref()),
                Some(to_x.as_ref()),
                anim,
            );
            let from_y = objc2_foundation::NSNumber::new_f64(old_sy);
            let to_y = objc2_foundation::NSNumber::new_f64(sy);
            crate::animation::apply_property_animation(
                &layer,
                "transform.scale.y",
                Some(from_y.as_ref()),
                Some(to_y.as_ref()),
                anim,
            );
        }
    }
}

/// Set the CALayer border color. No effect unless [`set_border_width`]
/// has been called with a width > 0.
pub fn set_border_color(node: &Node, color: crate::Color) {
    let view = node.ns_view();
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        let ns = color.to_nscolor();
        let new_cg = ns.CGColor();
        #[cfg(feature = "animation")]
        let old_cg = crate::animation::presentation_or_model(
            &layer, |l| l.borderColor()
        );
        layer.setBorderColor(Some(&new_cg));
        #[cfg(feature = "animation")]
        crate::animation::animate_color(
            &layer,
            "borderColor",
            old_cg.as_deref(),
            Some(&new_cg),
        );
    }
}
