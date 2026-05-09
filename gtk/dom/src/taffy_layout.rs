//! `TaffyLayout` — a `gtk::LayoutManager` subclass that delegates
//! measure + allocate to a [`taffy::TaffyTree`].
//!
//! Each container element ([`Element`](crate::Element) backing a
//! `<vstack>` / `<hstack>` / `<view>` / etc.) has one `TaffyLayout`
//! instance attached via `gtk::Widget::set_layout_manager`. Every
//! instance in a single window's tree shares the same `TreeRef` so
//! Taffy sees the whole hierarchy at once.
//!
//! Only the *root* container's `allocate` actually runs Taffy's
//! `compute_layout`; nested instances just look up their direct
//! children's pre-computed frames and call `child.allocate(...)`.
//!
//! Why a custom layout manager rather than `gtk::Box`? `BoxLayout`
//! does single-axis stacking only; for parity with the other native
//! ports we want the full flexbox feature set (gap, justify, align,
//! grow/shrink, percent sizes, …) and a single layout engine across
//! all platforms. Taffy delivers both.

use crate::layout::{
    measure_closure, AvailableSpace, Dimension, NodeId, Size, TreeRef,
};
use glib::subclass::prelude::*;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    pub struct TaffyLayout {
        /// The shared Taffy tree. `None` until `set_tree_node` is
        /// called (immediately after construction).
        pub tree: RefCell<Option<TreeRef>>,
        /// The Taffy node id corresponding to the widget this layout
        /// manager is attached to.
        pub node_id: RefCell<Option<NodeId>>,
        /// True for the root container (the window's content_root) —
        /// only it actually runs `compute_layout`.
        pub is_root: RefCell<bool>,
    }

    impl Default for TaffyLayout {
        fn default() -> Self {
            TaffyLayout {
                tree: RefCell::new(None),
                node_id: RefCell::new(None),
                is_root: RefCell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TaffyLayout {
        const NAME: &'static str = "LeptosTaffyLayout";
        type Type = super::TaffyLayout;
        type ParentType = gtk4::LayoutManager;
    }

    impl ObjectImpl for TaffyLayout {}

    impl LayoutManagerImpl for TaffyLayout {
        fn measure(
            &self,
            _widget: &gtk4::Widget,
            orientation: gtk4::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            // Walk Taffy for our node's natural size. If we haven't
            // computed yet (no allocate has run), Taffy's `layout()`
            // returns 0×0; report 0 in that case — GTK will call
            // allocate(width, height) with the parent's offered size
            // and we'll compute then.
            let Some(tree) = self.tree.borrow().clone() else {
                return (0, 0, -1, -1);
            };
            let Some(node_id) = *self.node_id.borrow() else {
                return (0, 0, -1, -1);
            };

            // Best-effort: measure the leaf-by-Taffy via
            // `compute_layout` against MaxContent so leaves return
            // their natural sizes. Only do this on the root —
            // measuring a nested container before allocate has reached
            // it would otherwise be expensive on every keystroke.
            if *self.is_root.borrow() {
                let mut tree_mut = tree.tree.borrow_mut();
                let _ = tree_mut.compute_layout_with_measure(
                    node_id,
                    Size {
                        width: AvailableSpace::MaxContent,
                        height: AvailableSpace::MaxContent,
                    },
                    measure_closure,
                );
                if let Ok(layout) = tree_mut.layout(node_id) {
                    let nat = match orientation {
                        gtk4::Orientation::Horizontal => {
                            layout.size.width as i32
                        }
                        _ => layout.size.height as i32,
                    };
                    return (0, nat.max(0), -1, -1);
                }
            }

            // Non-root: report whatever Taffy already cached (set when
            // the root last computed). Used only when GTK probes
            // children for their natural size; for our purposes the
            // value rarely matters because the parent's allocate will
            // overwrite the allocation anyway.
            if let Ok(layout) = tree.tree.borrow().layout(node_id) {
                let nat = match orientation {
                    gtk4::Orientation::Horizontal => {
                        layout.size.width as i32
                    }
                    _ => layout.size.height as i32,
                };
                return (0, nat.max(0), -1, -1);
            }
            (0, 0, -1, -1)
        }

        fn allocate(
            &self,
            widget: &gtk4::Widget,
            width: i32,
            height: i32,
            _baseline: i32,
        ) {
            let Some(tree) = self.tree.borrow().clone() else {
                return;
            };
            let Some(node_id) = *self.node_id.borrow() else {
                return;
            };

            if *self.is_root.borrow() {
                // Force the root to fill the available space exactly.
                let mut t = tree.tree.borrow_mut();
                let mut style = t
                    .style(node_id)
                    .cloned()
                    .unwrap_or_default();
                style.size = Size {
                    width: Dimension::length(width as f32),
                    height: Dimension::length(height as f32),
                };
                let _ = t.set_style(node_id, style);
                let _ = t.compute_layout_with_measure(
                    node_id,
                    Size {
                        width: AvailableSpace::Definite(width as f32),
                        height: AvailableSpace::Definite(height as f32),
                    },
                    measure_closure,
                );
            }

            // Walk our direct Taffy children, allocate each.
            allocate_children(&tree, node_id, widget);
        }
    }
}

glib::wrapper! {
    /// `gtk::LayoutManager` subclass that defers measure + allocate to
    /// Taffy. See module docs for the full picture.
    pub struct TaffyLayout(ObjectSubclass<imp::TaffyLayout>)
        @extends gtk4::LayoutManager;
}

impl TaffyLayout {
    pub fn new(tree: TreeRef, node_id: NodeId, is_root: bool) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        *imp.tree.borrow_mut() = Some(tree);
        *imp.node_id.borrow_mut() = Some(node_id);
        *imp.is_root.borrow_mut() = is_root;
        obj
    }
}

/// Look up each direct child of `parent_id` in the Taffy tree and
/// call `child_widget.allocate(width, height, -1, transform)` to
/// position it.
fn allocate_children(
    tree: &TreeRef,
    parent_id: NodeId,
    parent_widget: &gtk4::Widget,
) {
    // Snapshot the (child_id, child_widget, layout) triples. Each
    // `widget.allocate` call dispatches through GTK and may itself
    // re-enter our LayoutManagerImpl::allocate (for nested
    // containers), which borrows `tree.tree` again — so we release
    // the borrow before allocating.
    let plan: Vec<(NodeId, gtk4::Widget, taffy::Layout)> = {
        let t = tree.tree.borrow();
        t.children(parent_id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|cid| {
                let layout = *t.layout(cid).ok()?;
                let widget =
                    t.get_node_context(cid).map(|c| c.widget.clone())?;
                Some((cid, widget, layout))
            })
            .collect()
    };

    // GTK4 only allocates child widgets that are direct children of
    // the parent (per `child.parent() == Some(parent_widget)`). Our
    // Taffy tree mirrors the widget tree, so this is always true —
    // but we double-check to avoid abort()ing if a stale Taffy entry
    // refers to a now-detached widget.
    let parent_ptr = parent_widget.as_ptr();
    for (_cid, widget, layout) in plan {
        let parent_match =
            widget.parent().map(|p| p.as_ptr() == parent_ptr).unwrap_or(false);
        if !parent_match {
            continue;
        }
        let transform = gtk4::gsk::Transform::new().translate(
            &gtk4::graphene::Point::new(
                layout.location.x,
                layout.location.y,
            ),
        );
        let w = layout.size.width.max(0.0).round() as i32;
        let h = layout.size.height.max(0.0).round() as i32;
        widget.allocate(w, h, -1, Some(transform));
    }
}
