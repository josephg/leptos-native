//! GTK-side layout adapter.
//!
//! Tree storage and Taffy integration live in [`renderer`];
//! this file plugs GTK-specific types into it via [`GtkBackend`].
//! The shape mirrors `cocoa_dom::layout`: per-element wrappers
//! ([`set_as_root`], [`attach_child`], the `set_*` setters)
//! route through the new [`GtkElem`] accessors (`with_style`,
//! `tree_id`) and dispatch into
//! the shared tree.
//!
//! Where cocoa drives layout via an explicit `compute_layout` call
//! after every dirtying mutation, GTK piggybacks on its native
//! measure/allocate cycle: every dirtying mutation calls
//! [`queue_root_resize`], which asks GTK to re-run measure+allocate
//! on the next frame, and our [`super::taffy_layout::TaffyLayout`]
//! `LayoutManager` is what runs Taffy from inside that pass.

use crate::dom::node::GtkElem;
use gtk4::prelude::*;
use std::cell::RefCell;
use leptos_native::renderer;
pub use leptos_native::renderer::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, Layout, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Rect, Size, Style, TrackSizingFunction,
};
use leptos_native::renderer::{LayoutBackend, LayoutState};

// ---------------------------------------------------------------------
// GTK backend
// ---------------------------------------------------------------------

/// `LayoutBackend` impl for GTK4. The `View` is a `gtk::Widget`
/// (cheap-clonable reference). `NodeMeta = ()` — GTK has no
/// scroll-view second pass like cocoa does (NSScrollView's
/// content sizing); `gtk::ScrolledWindow` handles its own
/// content sizing.
pub struct GtkBackend;

thread_local! {
    /// The single per-thread node store for the gtk port.
    static TREE: RefCell<LayoutState<GtkBackend>> =
        RefCell::new(LayoutState::default());
}

impl LayoutBackend for GtkBackend {
    type View = gtk4::Widget;
    type NodeMeta = ();
    type Handlers = ();

    fn measure_leaf(
        widget: &Self::View,
        _meta: &Self::NodeMeta,
        known: Size<Option<f32>>,
        avail: Size<AvailableSpace>,
    ) -> Size<f32> {
        measure_leaf_size(widget, known, avail)
    }

    fn first_baseline(widget: &Self::View) -> Option<f32> {
        // GTK4 reports natural baseline through `measure(Vertical, -1)`.
        // -1 in the baseline slot means "no baseline".
        let (_, _, _, nat_baseline) =
            widget.measure(gtk4::Orientation::Vertical, -1);
        if nat_baseline >= 0 {
            Some(nat_baseline as f32)
        } else {
            None
        }
    }

    fn with_tree<R>(f: impl FnOnce(&mut LayoutState<Self>) -> R) -> R {
        TREE.with(|t| f(&mut t.borrow_mut()))
    }
}

pub type NodeContext = renderer::NodeContext<GtkBackend>;

// Introspection over the global store (used by tests).
// pub fn node_count() -> usize {
//     GtkBackend::node_count()
// }
// pub fn style(id: NodeId) -> Option<Style> {
//     GtkBackend::style(id)
// }
// pub fn children(id: NodeId) -> Vec<NodeId> {
//     renderer::children::<GtkBackend>(id)
// }
// pub fn dirty(id: NodeId) -> bool {
//     renderer::dirty::<GtkBackend>(id)
// }
// pub fn parent(id: NodeId) -> Option<NodeId> {
//     renderer::parent::<GtkBackend>(id)
// }
// pub fn contains(id: NodeId) -> bool {
//     renderer::contains::<GtkBackend>(id)
// }
// pub fn layout(id: NodeId) -> Option<Layout> {
//     renderer::layout::<GtkBackend>(id)
// }
// pub fn view(id: NodeId) -> Option<gtk4::Widget> {
//     renderer::view::<GtkBackend>(id)
// }
// pub fn remove(id: NodeId) {
//     renderer::remove::<GtkBackend>(id);
// }

// ---------------------------------------------------------------------
// Per-Node helpers — read/write Node state via its accessors.
// ---------------------------------------------------------------------

/// Drop the node (and its structural subtree) from the store and
/// unparent its widget, then queue a resize of the (former) parent's
/// root so its layout recomputes.
pub fn drop_node(node: impl std::borrow::Borrow<GtkElem>) {
    let node = *node.borrow();
    let parent = GtkBackend::parent(node.id());
    node.teardown();
    if let Some(pid) = parent {
        queue_root_resize_for(pid);
    }
}

// ---------------------------------------------------------------------
// Tree-edge mirroring
// ---------------------------------------------------------------------

pub fn attach_child(parent: impl std::borrow::Borrow<GtkElem>, child: impl std::borrow::Borrow<GtkElem>) {
    let parent_id = parent.borrow().id();
    GtkBackend::add_child(parent_id, child.borrow().id());
    queue_root_resize_for(parent_id);
}

pub fn insert_child_at(parent: impl std::borrow::Borrow<GtkElem>, child: impl std::borrow::Borrow<GtkElem>, index: usize) {
    let parent_id = parent.borrow().id();
    GtkBackend::insert_child_at_index(parent_id, index, child.borrow().id());
    queue_root_resize_for(parent_id);
}

/// Mirror a native insert-before-`marker` into Taffy by marker (no
/// native-order readback). `marker == None` appends. Canary counterpart to
/// [`insert_child_at`].
pub fn insert_child_before(
    parent: impl std::borrow::Borrow<GtkElem>,
    child: impl std::borrow::Borrow<GtkElem>,
    marker: Option<GtkElem>,
) {
    let parent_id = parent.borrow().id();
    GtkBackend::insert_child_before(parent_id, child.borrow().id(), marker.map(|m| m.id()));
    queue_root_resize_for(parent_id);
}

pub fn detach_child(parent: impl std::borrow::Borrow<GtkElem>, child: impl std::borrow::Borrow<GtkElem>) {
    let parent_id = parent.borrow().id();
    GtkBackend::remove_child(parent_id, child.borrow().id());
    queue_root_resize_for(parent_id);
}

/// Walk up from `id` to its subtree root and ask GTK to re-run
/// measure+allocate on that root's widget. `queue_resize` calls
/// coalesce into one pass per frame, so no extra dedup is needed.
fn queue_root_resize_for(id: NodeId) {
    let root = GtkBackend::root_of(id);
    if let Some(widget) = GtkBackend::view(root) {
        widget.queue_resize();
    }
    #[cfg(feature = "debug-overlay")]
    crate::dom::debug_overlay::mark_overlays_dirty();
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

pub fn update_style(node: GtkElem, f: impl FnOnce(&mut Style)) {
    node.with_style_mut(f);
}

pub fn set_style(node: GtkElem, style: Style) {
    update_style(node, |s| *s = style);
}

/// Mark this node dirty and queue a GTK resize of its root. Call after
/// content changes that affect intrinsic size (button title, label
/// text) so the cached measurement is invalidated.
pub fn schedule_relayout(node: GtkElem) {
    schedule_relayout_for(node.id());
}

/// [`schedule_relayout`] keyed by raw `NodeId`. Used by tooling (the
/// devtools inspector) that holds ids rather than `Node` handles.
pub fn schedule_relayout_for(id: NodeId) {
    GtkBackend::mark_dirty(id);
    queue_root_resize_for(id);
}

// ---------------------------------------------------------------------
// Generic style setters — lifted to `renderer::setters`. See the
// cocoa port's equivalent block for the design rationale.
// ---------------------------------------------------------------------

impl renderer::LayoutNodeOps for GtkElem {
    fn update_style<F: FnOnce(&mut Style)>(self, f: F) {
        update_style(self, f);
    }
    fn schedule_relayout(self) {
        schedule_relayout(self);
    }
    fn with_style<R, F: FnOnce(&Style) -> R>(self, f: F) -> R {
        GtkElem::with_style(self, f)
    }
}

impl renderer::LayoutElement for GtkElem {
    fn set_view_hidden(self, hidden: bool) {
        GtkElem::set_hidden(self, hidden);
    }
    fn set_clip(self, clip: bool) {
        use gtk4::prelude::WidgetExt;
        self.widget().set_overflow(if clip {
            gtk4::Overflow::Hidden
        } else {
            gtk4::Overflow::Visible
        });
    }
}
impl renderer::UniversalElement for GtkElem {
    fn set_alpha(self, alpha: f64) {
        GtkElem::set_alpha(self, alpha)
    }
    fn set_tool_tip(self, tip: &str) {
        GtkElem::set_tool_tip(self, tip)
    }
}

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

// ---------------------------------------------------------------------
// compute_layout (test helper)
// ---------------------------------------------------------------------
//
// At runtime, layout is driven by GTK's measure/allocate cycle through
// `taffy_layout::TaffyLayout`. For unit tests we want to compute
// layout against a fixed available size without running a GTK main
// loop; this helper mirrors `cocoa_dom::layout::compute_layout`.

pub fn compute_layout(root: impl std::borrow::Borrow<GtkElem>, available_size: (f32, f32)) {
    let root_id = root.borrow().id();
    if !GtkBackend::contains(root_id) {
        return;
    }
    let (w, h) = available_size;

    // For axes where the root's style.size is `auto`, fill the
    // available space. Explicit axes are left alone.
    {
        let mut style = GtkBackend::style(root_id).unwrap_or_default();
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
            GtkBackend::set_style(root_id, style);
        }
    }

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    GtkBackend::run_layout_pass(root_id, avail);
}

// ---------------------------------------------------------------------
// Leaf measure
// ---------------------------------------------------------------------

fn measure_leaf_size(
    widget: &gtk4::Widget,
    known: Size<Option<f32>>,
    avail: Size<AvailableSpace>,
) -> Size<f32> {
    if let (Some(w), Some(h)) = (known.width, known.height) {
        return Size { width: w, height: h };
    }

    let constraint_w = match (known.width, avail.width) {
        (Some(w), _) => Some(w as i32),
        (_, AvailableSpace::Definite(w)) => Some(w as i32),
        _ => None,
    };

    // Always ask for natural width with `-1` (unconstrained-height).
    // Passing `known.height` here would be telling GTK "what's the
    // width that fits in this height" — for wrap=true Labels that
    // wraps the text into the smallest column that fits vertically,
    // returning a width far below the text's natural extent. We want
    // the un-wrapped natural width; the parent's flex layout decides
    // whether to clamp it.
    let (_, nat_w, _, _) = widget.measure(gtk4::Orientation::Horizontal, -1);
    let mut w = known.width.unwrap_or(nat_w as f32);

    // For editable text entries, the natural width tracks content
    // (the entry grows as the user types), which would push the
    // field's frame outwards on every keystroke. Force width=0 so
    // the parent's flex layout decides — same trick cocoa_dom's
    // measure_leaf does for editable NSTextField. Sliders and
    // dropdowns have stable natural widths, so we leave them alone.
    if (widget.is::<gtk4::Entry>() || widget.is::<gtk4::PasswordEntry>())
        && known.width.is_none()
    {
        w = 0.0;
    }

    let height_for = constraint_w.unwrap_or(w as i32).max(-1);
    let (_, nat_h, _, _) =
        widget.measure(gtk4::Orientation::Vertical, height_for);
    let h = known.height.unwrap_or(nat_h as f32);

    Size { width: w.max(0.0), height: h.max(0.0) }
}
