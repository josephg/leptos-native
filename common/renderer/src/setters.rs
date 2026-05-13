//! Generic, port-agnostic style setters.
//!
//! Each native port (cocoa, iOS, GTK) needs the same set of helpers
//! for mutating a node's Taffy [`Style`] — `set_padding`,
//! `set_flex_grow`, `set_grid_template_columns`, etc. The mutations
//! themselves are identical across ports; only the surrounding glue
//! (`update_style` to write into the per-node slot + sync to the
//! tree, `schedule_relayout` to coalesce reflows onto one main-loop
//! tick) is port-specific.
//!
//! [`LayoutNodeOps`] captures the two port-specific operations. Each
//! port implements it for its `Node` type. Every other setter lives
//! here as a generic free function over `<N: LayoutNodeOps>`, so
//! adding (say) a new grid attribute is a one-line edit instead of
//! one edit per port.
//!
//! Pure renderer-type → taffy-type conversions ([`dim_to_dimension`],
//! [`align_self_to_taffy`], [`grid_line_to_placement`]) also live
//! here for the same reason.
//!
//! Per-port `layout.rs` re-exports the generic setters under their
//! short names so caller paths (`cocoa_dom::layout::set_padding(...)`,
//! `gtk_dom::layout::set_grid_template_columns(...)`) stay stable.

use crate::layout::{
    AlignContent, AlignItems, Dimension, FlexDirection, FlexWrap,
    GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, LengthPercentage, LengthPercentageAuto, Rect, Size, Style,
    TrackSizingFunction,
};
use crate::attrs::{
    install, AlignSelf, Dim, GridLine, LayoutAttrs, RenderEffect,
    UniversalAttrs,
};

// ---------------------------------------------------------------------
// Port-specific glue trait
// ---------------------------------------------------------------------

/// Implemented by each port's `Node` type. The two methods are the
/// only port-specific operations the generic setters below need:
///
/// - [`Self::update_style`] runs the given closure against the node's
///   local style copy, then syncs the result back into the layout
///   tree (if the node has joined one).
/// - [`Self::schedule_relayout`] queues a single layout pass for the
///   node's tree at the next main-loop tick, deduping with any other
///   queued pass.
///
/// All other setter behaviour — what fields to touch, what conversion
/// to apply — is identical across ports and lives in the free
/// functions below.
pub trait LayoutNodeOps {
    /// Mutate the node's style. The closure receives a `&mut Style`;
    /// changes are written back to the per-tree storage on return.
    fn update_style<F: FnOnce(&mut Style)>(&self, f: F);

    /// Mark this node (and its ancestors) dirty and queue a relayout
    /// pass for its tree.
    fn schedule_relayout(&self);
}

// ---------------------------------------------------------------------
// Pure conversions (renderer attr types → taffy types)
// ---------------------------------------------------------------------

/// `renderer::attrs::Dim` → taffy `Dimension`.
pub fn dim_to_dimension(d: Dim) -> Dimension {
    match d {
        Dim::Px(v) => Dimension::length(v),
        Dim::Pct(v) => Dimension::percent(v),
        Dim::Auto => Dimension::auto(),
    }
}

/// `renderer::attrs::AlignSelf` → taffy `AlignItems` (the type Taffy
/// uses for both align-items and align-self).
pub fn align_self_to_taffy(a: AlignSelf) -> Option<AlignItems> {
    match a {
        AlignSelf::Auto => None,
        AlignSelf::Start => Some(AlignItems::FlexStart),
        AlignSelf::End => Some(AlignItems::FlexEnd),
        AlignSelf::Center => Some(AlignItems::Center),
        AlignSelf::Stretch => Some(AlignItems::Stretch),
        AlignSelf::Baseline => Some(AlignItems::Baseline),
    }
}

/// `renderer::attrs::GridLine` → taffy `GridPlacement`.
pub fn grid_line_to_placement(line: GridLine) -> GridPlacement {
    match line {
        GridLine::Auto => GridPlacement::Auto,
        GridLine::Line(n) => taffy::style_helpers::line(n),
        GridLine::Span(n) => taffy::style_helpers::span(n),
    }
}

// ---------------------------------------------------------------------
// Sizing & box-model setters
// ---------------------------------------------------------------------

pub fn set_width<N: LayoutNodeOps>(node: &N, width_px: f32) {
    node.update_style(|s| s.size.width = Dimension::length(width_px));
    node.schedule_relayout();
}

pub fn set_height<N: LayoutNodeOps>(node: &N, height_px: f32) {
    node.update_style(|s| s.size.height = Dimension::length(height_px));
    node.schedule_relayout();
}

pub fn set_min_width<N: LayoutNodeOps>(node: &N, px: f32) {
    node.update_style(|s| s.min_size.width = Dimension::length(px));
    node.schedule_relayout();
}

pub fn set_max_width<N: LayoutNodeOps>(node: &N, px: f32) {
    node.update_style(|s| s.max_size.width = Dimension::length(px));
    node.schedule_relayout();
}

pub fn set_min_height<N: LayoutNodeOps>(node: &N, px: f32) {
    node.update_style(|s| s.min_size.height = Dimension::length(px));
    node.schedule_relayout();
}

pub fn set_max_height<N: LayoutNodeOps>(node: &N, px: f32) {
    node.update_style(|s| s.max_size.height = Dimension::length(px));
    node.schedule_relayout();
}

pub fn set_padding<N: LayoutNodeOps>(node: &N, all_px: f32) {
    node.update_style(|s| {
        s.padding = Rect {
            left: LengthPercentage::length(all_px),
            right: LengthPercentage::length(all_px),
            top: LengthPercentage::length(all_px),
            bottom: LengthPercentage::length(all_px),
        };
    });
    node.schedule_relayout();
}

pub fn set_margin<N: LayoutNodeOps>(node: &N, all_px: f32) {
    node.update_style(|s| {
        s.margin = Rect {
            left: LengthPercentageAuto::length(all_px),
            right: LengthPercentageAuto::length(all_px),
            top: LengthPercentageAuto::length(all_px),
            bottom: LengthPercentageAuto::length(all_px),
        };
    });
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Flex (container + item) setters
// ---------------------------------------------------------------------

pub fn set_flex_direction<N: LayoutNodeOps>(node: &N, dir: FlexDirection) {
    node.update_style(|s| s.flex_direction = dir);
    node.schedule_relayout();
}

pub fn set_flex_wrap<N: LayoutNodeOps>(node: &N, fw: FlexWrap) {
    node.update_style(|s| s.flex_wrap = fw);
    node.schedule_relayout();
}

pub fn set_justify_content<N: LayoutNodeOps>(node: &N, jc: JustifyContent) {
    node.update_style(|s| s.justify_content = Some(jc));
    node.schedule_relayout();
}

pub fn set_align_items<N: LayoutNodeOps>(node: &N, ai: AlignItems) {
    node.update_style(|s| s.align_items = Some(ai));
    node.schedule_relayout();
}

pub fn set_flex_grow<N: LayoutNodeOps>(node: &N, grow: f32) {
    node.update_style(|s| s.flex_grow = grow);
    node.schedule_relayout();
}

pub fn set_flex_shrink<N: LayoutNodeOps>(node: &N, shrink: f32) {
    node.update_style(|s| s.flex_shrink = shrink);
    node.schedule_relayout();
}

pub fn set_flex_basis<N: LayoutNodeOps>(node: &N, basis_px: f32) {
    node.update_style(|s| s.flex_basis = Dimension::length(basis_px));
    node.schedule_relayout();
}

pub fn set_align_self<N: LayoutNodeOps>(node: &N, ai: Option<AlignItems>) {
    node.update_style(|s| s.align_self = ai);
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Gap (shared by flex & grid; shorthand sets both axes)
// ---------------------------------------------------------------------

pub fn set_gap<N: LayoutNodeOps>(node: &N, gap_px: f32) {
    node.update_style(|s| {
        s.gap = Size {
            width: LengthPercentage::length(gap_px),
            height: LengthPercentage::length(gap_px),
        };
    });
    node.schedule_relayout();
}

pub fn set_column_gap<N: LayoutNodeOps>(node: &N, gap_px: f32) {
    node.update_style(|s| s.gap.width = LengthPercentage::length(gap_px));
    node.schedule_relayout();
}

pub fn set_row_gap<N: LayoutNodeOps>(node: &N, gap_px: f32) {
    node.update_style(|s| s.gap.height = LengthPercentage::length(gap_px));
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Grid container setters
// ---------------------------------------------------------------------

pub fn set_grid_template_columns<N: LayoutNodeOps>(
    node: &N,
    tracks: Vec<GridTemplateComponent>,
) {
    node.update_style(|s| s.grid_template_columns = tracks);
    node.schedule_relayout();
}

pub fn set_grid_template_rows<N: LayoutNodeOps>(
    node: &N,
    tracks: Vec<GridTemplateComponent>,
) {
    node.update_style(|s| s.grid_template_rows = tracks);
    node.schedule_relayout();
}

pub fn set_grid_auto_columns<N: LayoutNodeOps>(
    node: &N,
    tracks: Vec<TrackSizingFunction>,
) {
    node.update_style(|s| s.grid_auto_columns = tracks);
    node.schedule_relayout();
}

pub fn set_grid_auto_rows<N: LayoutNodeOps>(
    node: &N,
    tracks: Vec<TrackSizingFunction>,
) {
    node.update_style(|s| s.grid_auto_rows = tracks);
    node.schedule_relayout();
}

pub fn set_grid_auto_flow<N: LayoutNodeOps>(node: &N, flow: GridAutoFlow) {
    node.update_style(|s| s.grid_auto_flow = flow);
    node.schedule_relayout();
}

pub fn set_justify_items<N: LayoutNodeOps>(node: &N, ji: JustifyItems) {
    node.update_style(|s| s.justify_items = Some(ji));
    node.schedule_relayout();
}

pub fn set_align_content<N: LayoutNodeOps>(node: &N, ac: AlignContent) {
    node.update_style(|s| s.align_content = Some(ac));
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Grid item-side placement
// ---------------------------------------------------------------------

pub fn set_grid_column_start<N: LayoutNodeOps>(node: &N, line: GridLine) {
    let p = grid_line_to_placement(line);
    node.update_style(|s| s.grid_column.start = p);
    node.schedule_relayout();
}

pub fn set_grid_column_end<N: LayoutNodeOps>(node: &N, line: GridLine) {
    let p = grid_line_to_placement(line);
    node.update_style(|s| s.grid_column.end = p);
    node.schedule_relayout();
}

pub fn set_grid_row_start<N: LayoutNodeOps>(node: &N, line: GridLine) {
    let p = grid_line_to_placement(line);
    node.update_style(|s| s.grid_row.start = p);
    node.schedule_relayout();
}

pub fn set_grid_row_end<N: LayoutNodeOps>(node: &N, line: GridLine) {
    let p = grid_line_to_placement(line);
    node.update_style(|s| s.grid_row.end = p);
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Dim-variant sizing setters (used by the apply functions below;
// also useful directly when a builder wants to drive width/height
// from a `Dim` rather than a bare `f32`).
// ---------------------------------------------------------------------

pub fn set_size_width<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.size.width = dim_to_dimension(d));
    node.schedule_relayout();
}
pub fn set_size_height<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.size.height = dim_to_dimension(d));
    node.schedule_relayout();
}
pub fn set_min_size_width<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.min_size.width = dim_to_dimension(d));
    node.schedule_relayout();
}
pub fn set_min_size_height<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.min_size.height = dim_to_dimension(d));
    node.schedule_relayout();
}
pub fn set_max_size_width<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.max_size.width = dim_to_dimension(d));
    node.schedule_relayout();
}
pub fn set_max_size_height<N: LayoutNodeOps>(node: &N, d: Dim) {
    node.update_style(|s| s.max_size.height = dim_to_dimension(d));
    node.schedule_relayout();
}

// ---------------------------------------------------------------------
// Element-level traits — drive the reactive `apply_*` functions below.
// ---------------------------------------------------------------------

/// An element that owns (cheaply-cloneable handle to) a layout node.
/// Implemented per-port for `CocoaElement` / `IosElement` /
/// `GtkElement` so [`apply_layout`] can install reactive setters
/// generically.
pub trait LayoutElement: Clone + 'static {
    type Node: LayoutNodeOps;
    fn as_node(&self) -> &Self::Node;
}

/// Element-level handles for opacity + tooltip. Tooltip has a default
/// no-op implementation because iOS (and any future touch-only port)
/// has no hover-tooltip concept.
pub trait UniversalElement: Clone + 'static {
    fn set_alpha(&self, alpha: f64);
    fn set_tool_tip(&self, _tip: &str) {}
}

// ---------------------------------------------------------------------
// apply_layout — install reactive setters for every `LayoutAttrs`
// field that's been set. Returns the effects so the caller can stash
// them in its ElementState (they unsubscribe on drop).
// ---------------------------------------------------------------------

pub fn apply_layout<E>(
    el: &E,
    attrs: LayoutAttrs,
) -> Vec<RenderEffect<()>>
where
    E: LayoutElement,
{
    let mut out = Vec::new();

    // helper: install a reactive setter that drives `setter(node, v)`.
    macro_rules! install_setter {
        ($field:expr, $setter:expr) => {
            if let Some(v) = $field {
                let e = el.clone();
                if let Some(eff) = install(v, move |x| $setter(e.as_node(), x)) {
                    out.push(eff);
                }
            }
        };
    }

    install_setter!(attrs.padding, set_padding);
    install_setter!(attrs.margin, set_margin);
    install_setter!(attrs.width, set_size_width);
    install_setter!(attrs.height, set_size_height);
    install_setter!(attrs.min_width, set_min_size_width);
    install_setter!(attrs.min_height, set_min_size_height);
    install_setter!(attrs.max_width, set_max_size_width);
    install_setter!(attrs.max_height, set_max_size_height);
    install_setter!(attrs.flex_grow, set_flex_grow);

    // `align_self` converts `AlignSelf` → `Option<AlignItems>` before
    // applying; doesn't fit the bare-setter macro.
    if let Some(v) = attrs.align_self {
        let e = el.clone();
        if let Some(eff) = install(v, move |a: AlignSelf| {
            set_align_self(e.as_node(), align_self_to_taffy(a))
        }) {
            out.push(eff);
        }
    }

    install_setter!(attrs.grid_column_start, set_grid_column_start);
    install_setter!(attrs.grid_column_end, set_grid_column_end);
    install_setter!(attrs.grid_row_start, set_grid_row_start);
    install_setter!(attrs.grid_row_end, set_grid_row_end);

    out
}

// ---------------------------------------------------------------------
// apply_universal — opacity + tooltip.
// ---------------------------------------------------------------------

pub fn apply_universal<E>(
    el: &E,
    attrs: UniversalAttrs,
) -> Vec<RenderEffect<()>>
where
    E: UniversalElement,
{
    let mut out = Vec::new();
    if let Some(a) = attrs.alpha {
        let e = el.clone();
        if let Some(eff) = install(a, move |v| e.set_alpha(v)) {
            out.push(eff);
        }
    }
    if let Some(t) = attrs.tool_tip {
        let e = el.clone();
        if let Some(eff) = install(t, move |s: String| e.set_tool_tip(&s)) {
            out.push(eff);
        }
    }
    out
}
