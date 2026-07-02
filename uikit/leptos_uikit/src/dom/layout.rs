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

use crate::dom::node::{UikitElem, UikitNodeExt};
use dispatch2::DispatchQueue;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use objc2_ui_kit::{UIControl, UIScrollView, UITextField, UIView};
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::sync::OnceLock;
use taffy::Point;
pub use leptos_native::renderer::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, Layout, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Rect, Size, Style, TrackSizingFunction,
};
use leptos_native::renderer::{AttachOutcome, Backend, LayoutState};

pub use leptos_native::renderer::{
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
use crate::dom::event::IosNodeHandlers;
// ---------------------------------------------------------------------
// iOS backend
// ---------------------------------------------------------------------

/// `Backend` impl for UIKit. Same shape as `CocoaBackend` —
/// the `View` is a retained pointer to the per-node UIView; the
/// `NodeMeta` carries the scroll-view flag so the iOS-side
/// `compute_layout` knows to run a second pass on `<scroll_view>`
/// subtrees.
pub struct IosBackend;

#[derive(Clone, Default)]
pub struct IosMeta {
    pub is_scroll_view: bool,
}

thread_local! {
    /// The single per-thread node store for the uikit port.
    static TREE: RefCell<LayoutState<IosBackend>> =
        RefCell::new(LayoutState::default());
}

impl Backend for IosBackend {
    type View = SendWrapper<Retained<UIView>>;
    type NodeMeta = IosMeta;
    type Handlers = IosNodeHandlers;

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

    fn with_tree<R>(f: impl FnOnce(&mut LayoutState<Self>) -> R) -> R {
        TREE.with(|t| f(&mut t.borrow_mut()))
    }

    // Native view setters — forwarded to by the core `Node<B>` driver
    // blanket impls (orphan rule). iOS leaves `set_clip` / `set_tool_tip`
    // defaulted (no inline clip primitive / no tooltips on touch).

    fn set_hidden(view: &Self::View, hidden: bool) {
        let v: &UIView = view;
        if v.isHidden() != hidden {
            v.setHidden(hidden);
        }
    }

    fn set_alpha(view: &Self::View, alpha: f64) {
        let v: &UIView = view;
        let clamped = alpha.clamp(0.0, 1.0);
        if (v.alpha() - clamped).abs() > f64::EPSILON {
            v.setAlpha(clamped);
        }
    }

    fn schedule_relayout(id: NodeId) {
        IosBackend::mark_dirty(id);
        queue_relayout_for(id);
    }

    fn remove_from_native_parent(view: &Self::View) {
        let v: &UIView = view;
        v.removeFromSuperview();
    }

    fn create_text_node(text: &str) -> UikitElem {
        UikitElem::create_text(text)
    }

    fn create_placeholder() -> UikitElem {
        UikitElem::create_placeholder()
    }

    fn set_text(node: UikitElem, text: &str) {
        UikitNodeExt::set_text(node, text);
    }

    // Native tree edits. Native parent is `subview_parent()` (the
    // UIScrollView's content view for `<scroll_view>`); core mirrors the
    // edge into Taffy under `parent`. Marker-based — no subview-index
    // readback.

    fn attach_native(parent: NodeId, child: NodeId, before: Option<NodeId>) -> AttachOutcome {
        let p = UikitElem::from_id(parent);
        let c = UikitElem::from_id(child);
        let parent_retained = p.subview_parent();
        let parent_ref: &UIView = &parent_retained;
        let child_view = c.ui_view();
        match before.map(UikitElem::from_id) {
            None => parent_ref.addSubview(&child_view),
            Some(m) => {
                let marker_view = m.ui_view();
                parent_ref.insertSubview_belowSubview(&child_view, &marker_view);
            }
        }
        AttachOutcome::Mirror
    }

    fn detach_native(parent: NodeId, child: NodeId) -> bool {
        let p = UikitElem::from_id(parent);
        let c = UikitElem::from_id(child);
        let parent_retained = p.subview_parent();
        let parent_ptr: *const UIView = &*parent_retained;
        let child_view = c.ui_view();
        let same_parent = child_view
            .superview()
            .map(|sv| {
                let sv_ptr: *const UIView = &*sv;
                sv_ptr == parent_ptr
            })
            .unwrap_or(false);
        if !same_parent {
            return false;
        }
        child_view.removeFromSuperview();
        true
    }

    fn clear_native_children(parent: NodeId) {
        let p = UikitElem::from_id(parent);
        let parent_retained = p.subview_parent();
        let parent_ref: &UIView = &parent_retained;
        for sv in parent_ref.subviews().iter() {
            sv.removeFromSuperview();
        }
    }
}

pub type NodeContext = leptos_native::renderer::NodeContext<IosBackend>;

// Introspection over the global store (used by tests).
pub fn node_count() -> usize {
    IosBackend::node_count()
}
pub fn style(id: NodeId) -> Option<Style> {
    IosBackend::style(id)
}
pub fn children(id: NodeId) -> Vec<NodeId> {
    IosBackend::children(id)
}
pub fn dirty(id: NodeId) -> bool {
    IosBackend::dirty(id)
}
pub fn parent(id: NodeId) -> Option<NodeId> {
    IosBackend::parent(id)
}
pub fn contains(id: NodeId) -> bool {
    IosBackend::contains(id)
}
pub fn layout(id: NodeId) -> Option<Layout> {
    IosBackend::layout(id)
}
pub fn remove(id: NodeId) {
    IosBackend::remove(id);
}

// ---------------------------------------------------------------------
// Layout debug logging
// ---------------------------------------------------------------------

fn layout_debug_enabled() -> bool {
    static FLAG: OnceLock<bool> = OnceLock::new();
    *FLAG.get_or_init(|| std::env::var_os("IOS_DOM_LAYOUT_DEBUG").is_some())
}

// ---------------------------------------------------------------------
// Per-Node helpers — read/write Node state via its accessors
// ---------------------------------------------------------------------

// ---------------------------------------------------------------------
// Dynamic relayout — coalesce mutation bursts into one pass per tick.
// Each mutation enqueues the touched node; the deferred pass resolves
// each to its current subtree root and recomputes just those roots.
// ---------------------------------------------------------------------

pub fn schedule_relayout(node: UikitElem) {
    IosBackend::mark_dirty(node.id());
    queue_relayout_for(node.id());
}

/// Enqueue `id` for relayout on the next main-loop tick.
///
/// We enqueue the *touched node*, NOT its current subtree root. The
/// root is resolved (via `root_of`) when the pass drains — see
/// [`ensure_relayout_pass_scheduled`]. Resolving early is a bug:
/// reactive attr effects fire during `Render::build`, before the
/// node is mounted under `content_root`, so `root_of` would capture
/// an intermediate node as its own root. The deferred pass would
/// then recompute that intermediate as a root and plant it at Taffy
/// origin `(0,0)` — the "everything teleports to the top-left"
/// regression. Deferring `root_of` to drain time means the node is
/// attached by then and resolves to the real scene root.
fn queue_relayout_for(id: NodeId) {
    IosBackend::queue_relayout(id);
    ensure_relayout_pass_scheduled();
}

fn ensure_relayout_pass_scheduled() {
    if IosBackend::relayout_queued() {
        return;
    }
    IosBackend::set_relayout_queued(true);

    DispatchQueue::main().exec_async(move || {
        IosBackend::set_relayout_queued(false);
        // Resolve each touched node to its CURRENT subtree root (it's
        // attached by now, even if it wasn't when enqueued), dedup,
        // and recompute each unique root once. A node freed before now
        // isn't in the store and is skipped.
        let mut roots: Vec<NodeId> = Vec::new();
        for id in IosBackend::take_pending_relayout() {
            if !IosBackend::contains(id) {
                continue;
            }
            let root = IosBackend::root_of(id);
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
        for root_id in roots {
            let Some(view) = IosBackend::view(root_id) else {
                continue;
            };
            let root_view: Retained<UIView> = (*view).clone();
            let size = root_view.frame().size;
            compute_layout(UikitElem::from_id(root_id), size);
        }
    });
}

// ---------------------------------------------------------------------
// Tree-edge mirroring
// ---------------------------------------------------------------------

pub fn attach_child(parent: UikitElem, child: UikitElem) {
    let parent_id = parent.id();
    IosBackend::add_child(parent_id, child.id());
    queue_relayout_for(parent_id);
}

pub fn insert_child_at(parent: UikitElem, child: UikitElem, index: usize) {
    let parent_id = parent.id();
    IosBackend::insert_child_at_index(parent_id, index, child.id());
    queue_relayout_for(parent_id);
}

pub fn detach_child(parent: UikitElem, child: UikitElem) {
    let parent_id = parent.id();
    IosBackend::remove_child(parent_id, child.id());
    queue_relayout_for(parent_id);
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

pub fn update_style(node: UikitElem, f: impl FnOnce(&mut Style)) {
    node.with_style_mut(f);
}

pub fn set_style(node: UikitElem, style: Style) {
    update_style(node, |s| *s = style);
}

// ---------------------------------------------------------------------
// Layout computation
// ---------------------------------------------------------------------

pub fn compute_layout(root: UikitElem, available_size: NSSize) {
    if layout_debug_enabled() {
        eprintln!(
            "[compute_layout] avail {:.0}x{:.0}",
            available_size.width, available_size.height
        );
    }
    let root_id = root.id();
    if !IosBackend::contains(root_id) {
        return;
    }

    let w = available_size.width as f32;
    let h = available_size.height as f32;

    // For axes where the root's style.size is `auto`, fill the
    // available space. Explicit axes are left alone.
    {
        let mut style = IosBackend::style(root_id).unwrap_or_default();
        let mut touched = false;
        if style.size.width == Dimension::auto() {
            style.size.width = Dimension::length(w);
            touched = true;
        }
        if style.size.height == Dimension::auto() {
            style.size.height = Dimension::length(h);
            touched = true;
        }
        if touched {
            IosBackend::set_style(root_id, style);
        }
    }

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    IosBackend::run_layout_pass(root_id, avail);

    // iOS-specific: scroll-view second pass.
    relayout_scroll_views(root_id);

    apply_frames(root_id);
    fixup_scroll_view_contents(root_id);
}

fn is_scroll_view(id: NodeId) -> bool {
    IosBackend::meta(id).map(|m| m.is_scroll_view).unwrap_or(false)
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
    root: NodeId,
    viewport: &taffy::Layout,
) {
    use std::sync::Once;
    static WARNED: Once = Once::new();

    if viewport.size.height < 0.5
        && !IosBackend::children(root).is_empty()
    {
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

fn relayout_scroll_views(root: NodeId) {
    if is_scroll_view(root) {
        let viewport = match IosBackend::layout(root) {
            Some(l) => l,
            None => return,
        };
        warn_if_scroll_view_unsized(root, &viewport);
        let viewport_w = viewport.size.width;

        let saved_style = match IosBackend::style(root) {
            Some(s) => s,
            None => return,
        };
        let mut probe_style = saved_style.clone();
        probe_style.size = Size {
            width: Dimension::length(viewport_w),
            height: Dimension::auto(),
        };
        IosBackend::set_style(root, probe_style);
        IosBackend::mark_dirty(root);

        let avail = Size {
            width: AvailableSpace::Definite(viewport_w),
            height: AvailableSpace::MaxContent,
        };
        IosBackend::run_layout_pass(root, avail);

        IosBackend::set_style(root, saved_style);
        IosBackend::mark_dirty(root);
        // Restore the scroll view's own final layout to the
        // first-pass viewport.
        IosBackend::set_final_layout(root, viewport);
        return;
    }

    for child in IosBackend::children(root) {
        relayout_scroll_views(child);
    }
}

fn apply_frames(id: NodeId) {
    for (_id, layout, view) in IosBackend::collect_subtree(id) {
        set_frame_from_layout(&view, &layout);
    }
}

/// Bound each `<scroll_view>` content view to its children's natural
/// extent, and update the UIScrollView's `contentSize` so it scrolls.
fn fixup_scroll_view_contents(root: NodeId) {
    if is_scroll_view(root) {
        let Some(view) = IosBackend::view(root) else { return };
        let uiview: &UIView = &**view;
        let any: &AnyObject = uiview.as_ref();
        if let Some(scroll) = any.downcast_ref::<UIScrollView>() {
            let viewport = IosBackend::layout(root).unwrap_or_default();
            let mut max_x: f32 = 0.0;
            let mut max_y: f32 = 0.0;
            for child_id in IosBackend::children(root) {
                let Some(cl) = IosBackend::layout(child_id) else {
                    continue;
                };
                max_x = max_x.max(cl.location.x + cl.size.width);
                max_y = max_y.max(cl.location.y + cl.size.height);
            }
            let cw = (max_x as f64).max(viewport.size.width as f64);
            let ch = (max_y as f64).max(viewport.size.height as f64);
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

    for child in IosBackend::children(root) {
        fixup_scroll_view_contents(child);
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

// The per-port `LayoutNodeOps` / `LayoutElement` / `UniversalElement` impls
// that used to live here are gone: with `UikitElem` now an alias for the
// foreign `Node<IosBackend>`, impl'ing those (foreign, param-less) traits
// here is an orphan violation. They're blanket-impl'd in core for `Node<B>`,
// forwarding to the `Backend` native-setter hooks (`set_hidden`,
// `set_alpha`, `schedule_relayout`) on `IosBackend` above. iOS leaves
// `set_clip` / `set_tool_tip` defaulted (no inline clip primitive / no
// hover tooltips on touch).

// ---------------------------------------------------------------------
// iOS-only setters — Taffy fields the cocoa port doesn't currently
// expose (aspect_ratio, position, inset), used by the UIView builders
// for square photo cells and overlay badges.
// ---------------------------------------------------------------------

/// Force a node's `aspect_ratio` (width / height). Useful for
/// square photo cells (`aspect_ratio = 1.0`).
pub fn set_aspect_ratio(node: UikitElem, ratio: f32) {
    update_style(node, |s| s.aspect_ratio = Some(ratio));
    schedule_relayout(node);
}

/// Set Taffy's `position` flag. `Position::Absolute` removes the
/// node from the parent's flex layout — it positions itself
/// relative to the parent's content area using `inset_*`. Used for
/// overlay badges.
pub fn set_position(node: UikitElem, position: Position) {
    update_style(node, |s| s.position = position);
    schedule_relayout(node);
}

/// Set the four insets at once. Each value is points; `None`
/// leaves that side as `Auto`. With `Position::Absolute`, an inset
/// of 0 anchors that edge to the parent's content edge.
pub fn set_inset(
    node: UikitElem,
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
