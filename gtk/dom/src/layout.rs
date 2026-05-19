//! GTK-side layout adapter.
//!
//! Tree storage and Taffy integration live in [`renderer`];
//! this file plugs GTK-specific types into it via [`GtkBackend`].
//! The shape mirrors `cocoa_dom::layout`: per-element wrappers
//! ([`set_as_root`], [`attach_child`], the `set_*` setters)
//! route through the new [`Node`] accessors (`with_style`,
//! `tree_id`) and dispatch into
//! the shared tree.
//!
//! Where cocoa drives layout via an explicit `compute_layout` call
//! after every dirtying mutation, GTK piggybacks on its native
//! measure/allocate cycle: every dirtying mutation calls
//! [`queue_root_resize`], which asks GTK to re-run measure+allocate
//! on the next frame, and our [`super::taffy_layout::TaffyLayout`]
//! `LayoutManager` is what runs Taffy from inside that pass.

use crate::node::Node;
use gtk4::prelude::*;
use std::rc::Rc;

pub use renderer::{
    AlignContent, AlignItems, AvailableSpace, Dimension, Display, FlexDirection,
    FlexWrap, GridAutoFlow, GridPlacement, GridTemplateComponent, JustifyContent,
    JustifyItems, Layout, LengthPercentage, LengthPercentageAuto, NodeId,
    Position, Rect, Size, Style, TrackSizingFunction,
};
use renderer::LayoutBackend;

// ---------------------------------------------------------------------
// GTK backend
// ---------------------------------------------------------------------

/// `LayoutBackend` impl for GTK4. The `View` is a `gtk::Widget`
/// (cheap-clonable reference). `NodeMeta = ()` — GTK has no
/// scroll-view second pass like cocoa does (NSScrollView's
/// content sizing); `gtk::ScrolledWindow` handles its own
/// content sizing.
pub struct GtkBackend;

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
}

// Aliases so call sites don't have to spell `GtkBackend` everywhere.
pub type LayoutTree = renderer::LayoutTree<GtkBackend>;
pub type TreeRef = renderer::TreeRef<GtkBackend>;
pub type LayoutHandle = renderer::LayoutHandle<GtkBackend>;
pub type NodeContext = renderer::NodeContext<GtkBackend>;

pub fn new_tree() -> TreeRef {
    LayoutTree::new()
}

// ---------------------------------------------------------------------
// Per-Node helpers — read/write Node state via its accessors.
// ---------------------------------------------------------------------

/// Mark `node` as the tree's root, if no root is set yet.
///
/// Arena allocation happens eagerly in `Element::create` (and
/// `Text` / `Placeholder`), so this no longer does any registration
/// work. It just publishes the node id as `tree.root` — which the
/// GTK measure/allocate driver reads to find what to lay out. If
/// a root is already set, this is a silent no-op (first wins).
pub fn set_as_root(node: &Node, tree: &TreeRef) {
    let Some((node_tree, id)) = node.tree_id() else { return };
    debug_assert!(
        Rc::ptr_eq(&node_tree, tree),
        "set_as_root called with a tree that doesn't own this node"
    );
    let mut root = tree.root.borrow_mut();
    if root.is_none() {
        *root = Some(id);
    }
}

/// Drop the node and detach it from the tree. Now that `NodeInner::Drop`
/// already calls `decref`, this just exists for callers that want an
/// eager detach-now point and to mark the (former) parent dirty.
pub fn drop_node(node: &Node) {
    let parent = node
        .tree_id()
        .and_then(|(tree, id)| tree.parent(id).map(|pid| (tree, pid)));
    // Force an eager arena removal (bypass refcount > 0 GC).
    node.teardown();
    if let Some((tree, pid)) = parent {
        tree.mark_dirty(pid);
        queue_root_resize(&tree);
    }
}

// ---------------------------------------------------------------------
// Tree-edge mirroring
// ---------------------------------------------------------------------

pub fn attach_child(parent: &Node, child: &Node) {
    let Some((tree, parent_id)) = parent.tree_id() else { return };
    let Some((_, child_id)) = child.tree_id() else { return };
    tree.add_child(parent_id, child_id);
    queue_root_resize(&tree);
}

pub fn insert_child_at(parent: &Node, child: &Node, index: usize) {
    let Some((tree, parent_id)) = parent.tree_id() else { return };
    let Some((_, child_id)) = child.tree_id() else { return };
    tree.insert_child_at_index(parent_id, index, child_id);
    queue_root_resize(&tree);
}

pub fn detach_child(parent: &Node, child: &Node) {
    let Some((tree, parent_id)) = parent.tree_id() else { return };
    let Some((_, child_id)) = child.tree_id() else { return };
    tree.remove_child(parent_id, child_id);
    queue_root_resize(&tree);
}

/// Ask GTK to re-run measure+allocate on the tree's root widget.
/// Each TaffyLayout instance registers the root widget on the tree
/// so we can fish it out via the stored root NodeId. Multiple
/// `queue_resize` calls coalesce into one pass per frame.
pub fn queue_root_resize(tree: &TreeRef) {
    let Some(root_id) = *tree.root.borrow() else { return };
    if let Some(widget) = tree.view(root_id) {
        widget.queue_resize();
    }
    #[cfg(feature = "debug-overlay")]
    crate::debug_overlay::mark_overlays_dirty();
}

// ---------------------------------------------------------------------
// Style mutation
// ---------------------------------------------------------------------

pub fn update_style(node: &Node, f: impl FnOnce(&mut Style)) {
    // `Node::with_style_mut` reads the style from the arena, runs
    // `f`, pushes the updated style back via `tree.set_style`
    // (which marks the node dirty for the next Taffy pass).
    node.with_style_mut(f);
}

pub fn set_style(node: &Node, style: Style) {
    update_style(node, |s| *s = style);
}

/// Mark this node dirty in the tree and queue a GTK resize. Call
/// after content changes that affect intrinsic size (button title,
/// label text). GTK already calls `queue_resize` for us when
/// changing the corresponding widget property — but we still need
/// to mark the cached measurement invalid.
pub fn schedule_relayout(node: &Node) {
    if let Some((tree, id)) = node.tree_id() {
        tree.mark_dirty(id);
        queue_root_resize(&tree);
    }
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
        Node::with_style(self, f)
    }
}

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
        use gtk4::prelude::WidgetExt;
        let widget = self.as_node().widget();
        widget.set_overflow(if clip {
            gtk4::Overflow::Hidden
        } else {
            gtk4::Overflow::Visible
        });
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

pub fn compute_layout(root: &Node, available_size: (f32, f32)) {
    let Some(handle) = root.mounted_handle() else { return };
    let (w, h) = available_size;

    // For axes where the root's style.size is `auto`, fill the
    // available space. Axes the user has set explicitly are left
    // alone — see cocoa's compute_layout for the rationale.
    {
        let mut style = handle.tree.style(handle.node_id).unwrap_or_default();
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
            handle.tree.set_style(handle.node_id, style);
        }
    }

    let avail = Size {
        width: AvailableSpace::Definite(w),
        height: AvailableSpace::Definite(h),
    };
    handle.tree.run_layout_pass(handle.node_id, avail);
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
