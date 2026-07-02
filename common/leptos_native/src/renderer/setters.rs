//! Generic, port-agnostic style setters.
//!
//! Each native port (cocoa, iOS, GTK) needs the same set of helpers
//! for mutating a node's Taffy [`Style`] — `set_padding`,
//! `set_flex_grow`, `set_grid_template_columns`, etc. The mutations
//! are identical across ports, so they live here once as free
//! functions over the universal handle [`Node<B>`]. Each one writes
//! the style change into the store and schedules a (deduped) relayout
//! pass for the node's tree.
//!
//! Pure renderer-type → taffy-type conversions ([`dim_to_dimension`],
//! [`align_self_to_taffy`], [`grid_line_to_placement`]) also live
//! here for the same reason.
//!
//! The reactive install loops [`apply_layout`] / [`apply_universal`] /
//! [`apply_decoration`] read the builder-side attr structs
//! ([`LayoutAttrs`], …) and install a setter (static) or a
//! `RenderEffect` (reactive) per populated field. Platform-visual
//! side effects (hide the view, clip, alpha, tooltip, decoration)
//! route through the [`Backend`] native-setter hooks, always via
//! [`Node::try_view`] so an effect that fires after its node was torn
//! down is a graceful no-op.

use crate::renderer::attrs::{
    install, AlignSelf, DecorationAttrs, Dim, Edges, GridLine, LayoutAttrs,
    Overflow, RenderEffect, UniversalAttrs,
};
use crate::renderer::node::Node;
use crate::renderer::scene::{
    AlignContent, AlignItems, Backend, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent,
    JustifyContent, JustifyItems, LengthPercentage, LengthPercentageAuto,
    Rect, Size, Style, TrackSizingFunction,
};

/// Apply a style mutation and schedule a relayout — the shared tail of
/// every setter below.
fn style<B: Backend>(node: Node<B>, f: impl FnOnce(&mut Style)) {
    node.with_style_mut(f);
    node.schedule_relayout();
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

pub fn set_width<B: Backend>(node: Node<B>, width_px: f32) {
    style(node, |s| s.size.width = Dimension::length(width_px));
}

pub fn set_height<B: Backend>(node: Node<B>, height_px: f32) {
    style(node, |s| s.size.height = Dimension::length(height_px));
}

pub fn set_min_width<B: Backend>(node: Node<B>, px: f32) {
    style(node, |s| s.min_size.width = Dimension::length(px));
}

pub fn set_max_width<B: Backend>(node: Node<B>, px: f32) {
    style(node, |s| s.max_size.width = Dimension::length(px));
}

pub fn set_min_height<B: Backend>(node: Node<B>, px: f32) {
    style(node, |s| s.min_size.height = Dimension::length(px));
}

pub fn set_max_height<B: Backend>(node: Node<B>, px: f32) {
    style(node, |s| s.max_size.height = Dimension::length(px));
}

pub fn set_padding<B: Backend>(node: Node<B>, e: impl Into<Edges>) {
    let e = e.into();
    style(node, |s| {
        s.padding = Rect {
            left:   LengthPercentage::length(e.left),
            right:  LengthPercentage::length(e.right),
            top:    LengthPercentage::length(e.top),
            bottom: LengthPercentage::length(e.bottom),
        };
    });
}

pub fn set_margin<B: Backend>(node: Node<B>, e: impl Into<Edges>) {
    let e = e.into();
    style(node, |s| {
        s.margin = Rect {
            left:   LengthPercentageAuto::length(e.left),
            right:  LengthPercentageAuto::length(e.right),
            top:    LengthPercentageAuto::length(e.top),
            bottom: LengthPercentageAuto::length(e.bottom),
        };
    });
}

// ---------------------------------------------------------------------
// Flex (container + item) setters
// ---------------------------------------------------------------------

pub fn set_flex_direction<B: Backend>(node: Node<B>, dir: FlexDirection) {
    style(node, |s| s.flex_direction = dir);
}

pub fn set_flex_wrap<B: Backend>(node: Node<B>, fw: FlexWrap) {
    style(node, |s| s.flex_wrap = fw);
}

pub fn set_justify_content<B: Backend>(node: Node<B>, jc: JustifyContent) {
    style(node, |s| s.justify_content = Some(jc));
}

pub fn set_align_items<B: Backend>(node: Node<B>, ai: AlignItems) {
    style(node, |s| s.align_items = Some(ai));
}

pub fn set_flex_grow<B: Backend>(node: Node<B>, grow: f32) {
    style(node, |s| s.flex_grow = grow);
}

pub fn set_flex_shrink<B: Backend>(node: Node<B>, shrink: f32) {
    style(node, |s| s.flex_shrink = shrink);
}

pub fn set_flex_basis<B: Backend>(node: Node<B>, basis_px: f32) {
    style(node, |s| s.flex_basis = Dimension::length(basis_px));
}

pub fn set_align_self<B: Backend>(node: Node<B>, ai: Option<AlignItems>) {
    style(node, |s| s.align_self = ai);
}

// ---------------------------------------------------------------------
// Gap (shared by flex & grid; shorthand sets both axes)
// ---------------------------------------------------------------------

pub fn set_gap<B: Backend>(node: Node<B>, gap_px: f32) {
    style(node, |s| {
        s.gap = Size {
            width: LengthPercentage::length(gap_px),
            height: LengthPercentage::length(gap_px),
        };
    });
}

pub fn set_column_gap<B: Backend>(node: Node<B>, gap_px: f32) {
    style(node, |s| s.gap.width = LengthPercentage::length(gap_px));
}

pub fn set_row_gap<B: Backend>(node: Node<B>, gap_px: f32) {
    style(node, |s| s.gap.height = LengthPercentage::length(gap_px));
}

// ---------------------------------------------------------------------
// Grid container setters
// ---------------------------------------------------------------------

pub fn set_grid_template_columns<B: Backend>(
    node: Node<B>,
    tracks: Vec<GridTemplateComponent>,
) {
    style(node, |s| s.grid_template_columns = tracks);
}

pub fn set_grid_template_rows<B: Backend>(
    node: Node<B>,
    tracks: Vec<GridTemplateComponent>,
) {
    style(node, |s| s.grid_template_rows = tracks);
}

pub fn set_grid_auto_columns<B: Backend>(
    node: Node<B>,
    tracks: Vec<TrackSizingFunction>,
) {
    style(node, |s| s.grid_auto_columns = tracks);
}

pub fn set_grid_auto_rows<B: Backend>(
    node: Node<B>,
    tracks: Vec<TrackSizingFunction>,
) {
    style(node, |s| s.grid_auto_rows = tracks);
}

pub fn set_grid_auto_flow<B: Backend>(node: Node<B>, flow: GridAutoFlow) {
    style(node, |s| s.grid_auto_flow = flow);
}

pub fn set_justify_items<B: Backend>(node: Node<B>, ji: JustifyItems) {
    style(node, |s| s.justify_items = Some(ji));
}

pub fn set_align_content<B: Backend>(node: Node<B>, ac: AlignContent) {
    style(node, |s| s.align_content = Some(ac));
}

// ---------------------------------------------------------------------
// Grid item-side placement
// ---------------------------------------------------------------------

pub fn set_grid_column_start<B: Backend>(node: Node<B>, line: GridLine) {
    let p = grid_line_to_placement(line);
    style(node, |s| s.grid_column.start = p);
}

pub fn set_grid_column_end<B: Backend>(node: Node<B>, line: GridLine) {
    let p = grid_line_to_placement(line);
    style(node, |s| s.grid_column.end = p);
}

pub fn set_grid_row_start<B: Backend>(node: Node<B>, line: GridLine) {
    let p = grid_line_to_placement(line);
    style(node, |s| s.grid_row.start = p);
}

pub fn set_grid_row_end<B: Backend>(node: Node<B>, line: GridLine) {
    let p = grid_line_to_placement(line);
    style(node, |s| s.grid_row.end = p);
}

// ---------------------------------------------------------------------
// Dim-variant sizing setters (used by the apply functions below;
// also useful directly when a builder wants to drive width/height
// from a `Dim` rather than a bare `f32`).
// ---------------------------------------------------------------------

pub fn set_size_width<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.size.width = dim_to_dimension(d));
}
pub fn set_size_height<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.size.height = dim_to_dimension(d));
}
pub fn set_min_size_width<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.min_size.width = dim_to_dimension(d));
}
pub fn set_min_size_height<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.min_size.height = dim_to_dimension(d));
}
pub fn set_max_size_width<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.max_size.width = dim_to_dimension(d));
}
pub fn set_max_size_height<B: Backend>(node: Node<B>, d: Dim) {
    style(node, |s| s.max_size.height = dim_to_dimension(d));
}

// ---------------------------------------------------------------------
// Display (used by the `hidden=` attr — display: none collapses the
// slot in Taffy in addition to whatever visual hiding the port does).
// ---------------------------------------------------------------------

pub fn set_display<B: Backend>(node: Node<B>, display: Display) {
    style(node, |s| s.display = display);
}

/// Sets Taffy `style.overflow` on both axes. The user-facing
/// [`Overflow`] enum is whole-element (no per-axis distinction);
/// add a per-axis API later if needed.
///
/// `Clip` maps to taffy's `Clip`, which keeps the node's
/// auto-min-size content-based — same effect on the flex/grid auto-min
/// rule as `Visible`. `Hidden` maps to taffy's `Hidden`, which forces
/// auto-min-size to 0.
pub fn set_overflow<B: Backend>(node: Node<B>, overflow: Overflow) {
    let v = match overflow {
        Overflow::Visible => taffy::Overflow::Visible,
        Overflow::Clip    => taffy::Overflow::Clip,
        Overflow::Hidden  => taffy::Overflow::Hidden,
    };
    style(node, |s| s.overflow = taffy::Point { x: v, y: v });
}

// ---------------------------------------------------------------------
// apply_decoration — background_color, corner_radius, border. Routes
// to the Backend decoration hooks (no-op defaults for ports without
// layer-backed chrome).
// ---------------------------------------------------------------------

pub fn apply_decoration<B: Backend>(
    el: Node<B>,
    attrs: DecorationAttrs<B::Color>,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();

    macro_rules! install_setter {
        ($field:expr, $hook:ident) => {
            if let Some(v) = $field {
                if let Some(eff) = install(v, move |x| {
                    if let Some(view) = el.try_view() {
                        B::$hook(&view, x);
                    }
                }) {
                    out.push(eff);
                }
            }
        };
    }

    install_setter!(attrs.background_color, set_background_color);
    install_setter!(attrs.corner_radius,    set_corner_radius);
    install_setter!(attrs.border_width,     set_border_width);
    install_setter!(attrs.border_color,     set_border_color);
    out
}

// ---------------------------------------------------------------------
// apply_layout — install reactive setters for every `LayoutAttrs`
// field that's been set. Returns the effects so the caller can stash
// them in its ElementState (they unsubscribe on drop).
// ---------------------------------------------------------------------

pub fn apply_layout<B: Backend>(
    el: Node<B>,
    attrs: LayoutAttrs,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();

    // helper: install a reactive setter that drives `setter(node, v)`.
    macro_rules! install_setter {
        ($field:expr, $setter:expr) => {
            if let Some(v) = $field {
                if let Some(eff) = install(v, move |x| $setter(el, x)) {
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
    install_setter!(attrs.flex_grow,   set_flex_grow);
    install_setter!(attrs.flex_shrink, set_flex_shrink);
    install_setter!(attrs.flex_basis,  set_flex_basis);

    // `align_self` converts `AlignSelf` → `Option<AlignItems>` before
    // applying; doesn't fit the bare-setter macro.
    if let Some(v) = attrs.align_self {
        if let Some(eff) = install(v, move |a: AlignSelf| {
            set_align_self(el, align_self_to_taffy(a))
        }) {
            out.push(eff);
        }
    }

    install_setter!(attrs.grid_column_start, set_grid_column_start);
    install_setter!(attrs.grid_column_end, set_grid_column_end);
    install_setter!(attrs.grid_row_start, set_grid_row_start);
    install_setter!(attrs.grid_row_end, set_grid_row_end);

    // `overflow=` ⇒ Taffy style.overflow (auto-min-size effect for
    // Hidden) **and** the port-side visual clip. The mapping:
    //   Visible → Taffy Visible, clip off
    //   Clip    → Taffy Clip,    clip on   (paint-only clipping)
    //   Hidden  → Taffy Hidden,  clip on   (clip + auto-min-size 0)
    if let Some(v) = attrs.overflow {
        if let Some(eff) = install(v, move |o: Overflow| {
            set_overflow(el, o);
            if let Some(view) = el.try_view() {
                B::set_clip(&view, !matches!(o, Overflow::Visible));
            }
        }) {
            out.push(eff);
        }
    }

    // `hidden=true` ⇒ Taffy `Display::None` (collapses the slot).
    // `hidden=false` ⇒ restore whatever display the node was created
    // with (Flex / Grid / Block / None). Capture the natural display
    // once at install time.
    if let Some(v) = attrs.hidden {
        let natural = el.with_style(|s| s.display);
        if let Some(eff) = install(v, move |hide: bool| {
            let next = if hide { Display::None } else { natural };
            set_display(el, next);
            // The port also updates view-level visibility (NSView /
            // UIView isHidden, gtk set_visible) so the actual pixels
            // go away when the slot collapses to zero size.
            if let Some(view) = el.try_view() {
                B::set_hidden(&view, hide);
            }
        }) {
            out.push(eff);
        }
    }

    out
}

// ---------------------------------------------------------------------
// apply_universal — opacity + tooltip.
// ---------------------------------------------------------------------

pub fn apply_universal<B: Backend>(
    el: Node<B>,
    attrs: UniversalAttrs,
) -> Vec<RenderEffect<()>> {
    let mut out = Vec::new();
    if let Some(a) = attrs.alpha {
        if let Some(eff) = install(a, move |v| {
            if let Some(view) = el.try_view() {
                B::set_alpha(&view, v);
            }
        }) {
            out.push(eff);
        }
    }
    if let Some(t) = attrs.tool_tip {
        if let Some(eff) = install(t, move |s: String| {
            if let Some(view) = el.try_view() {
                B::set_tool_tip(&view, &s);
            }
        }) {
            out.push(eff);
        }
    }
    out
}
