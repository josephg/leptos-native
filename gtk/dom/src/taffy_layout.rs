//! `TaffyLayout` — a `gtk::LayoutManager` subclass that delegates
//! measure + allocate to our shared [`LayoutTree`].
//!
//! Each container element backing a `<vstack>` / `<hstack>` /
//! `<view>` / etc. has one `TaffyLayout` instance attached via
//! `gtk::Widget::set_layout_manager`. Every instance in a single
//! window's tree shares the same `TreeRef` so the layout engine
//! sees the whole hierarchy at once.
//!
//! Only the *root* container's `allocate` actually runs the layout
//! pass; nested instances just look up their direct children's
//! pre-computed frames and call `child.allocate(...)`.
//!
//! Why a custom layout manager rather than `gtk::Box`? `BoxLayout`
//! does single-axis stacking only; for parity with the other native
//! ports we want the full flexbox feature set (gap, justify, align,
//! grow/shrink, percent sizes, baseline alignment) and a single
//! layout engine across all platforms.

use crate::layout::{
    AvailableSpace, Dimension, NodeId, Size, TreeRef,
};
use glib::subclass::prelude::*;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    pub struct TaffyLayout {
        pub tree: RefCell<Option<TreeRef>>,
        pub node_id: RefCell<Option<NodeId>>,
        /// True for the root container (the window's content_root) —
        /// only it actually runs the layout pass.
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
            let Some(tree) = self.tree.borrow().clone() else {
                return (0, 0, -1, -1);
            };
            let Some(node_id) = *self.node_id.borrow() else {
                return (0, 0, -1, -1);
            };

            // Best-effort: probe leaves' natural size by running the
            // layout pass against `MaxContent`. Only do this on the
            // root — measuring a nested container before allocate
            // has reached it would be expensive on every keystroke.
            if *self.is_root.borrow() {
                tree.run_layout_pass(
                    node_id,
                    Size {
                        width: AvailableSpace::MaxContent,
                        height: AvailableSpace::MaxContent,
                    },
                );
                if let Some(layout) = tree.layout(node_id) {
                    let nat = match orientation {
                        gtk4::Orientation::Horizontal => layout.size.width as i32,
                        _ => layout.size.height as i32,
                    };
                    return (0, nat.max(0), -1, -1);
                }
            }

            // Non-root: report whatever the tree already cached. Used
            // only when GTK probes children for their natural size;
            // for our purposes the value rarely matters because the
            // parent's allocate will overwrite the allocation anyway.
            if let Some(layout) = tree.layout(node_id) {
                let nat = match orientation {
                    gtk4::Orientation::Horizontal => layout.size.width as i32,
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
            let Some(tree) = self.tree.borrow().clone() else { return };
            let Some(node_id) = *self.node_id.borrow() else { return };

            if *self.is_root.borrow() {
                // Force the root to fill the available space exactly
                // and run the layout pass.
                let mut style = tree.style(node_id).unwrap_or_default();
                style.size = Size {
                    width: Dimension::length(width as f32),
                    height: Dimension::length(height as f32),
                };
                tree.set_style(node_id, style);
                tree.run_layout_pass(
                    node_id,
                    Size {
                        width: AvailableSpace::Definite(width as f32),
                        height: AvailableSpace::Definite(height as f32),
                    },
                );
            }

            allocate_children(&tree, node_id, widget);
        }
    }
}

glib::wrapper! {
    /// `gtk::LayoutManager` subclass that defers measure + allocate
    /// to our [`LayoutTree`]. See module docs for the full picture.
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

/// Look up each direct child of `parent_id` in the tree and call
/// `widget.allocate(width, height, -1, transform)` to position it.
///
/// GTK4 only allocates child widgets that are direct children of
/// the parent (per `child.parent() == Some(parent_widget)`). Our
/// tree mirrors the widget tree, so this is always true — but we
/// double-check to avoid abort()ing if a stale Taffy entry refers
/// to a now-detached widget.
fn allocate_children(tree: &TreeRef, parent_id: NodeId, parent_widget: &gtk4::Widget) {
    // Snapshot (widget, layout) pairs. Each `widget.allocate(...)`
    // call dispatches through GTK and may itself re-enter our
    // LayoutManagerImpl::allocate (for nested containers); avoid
    // holding any tree borrows across the call.
    // Snapshot child IDs to drop the `Ref` from `children()` before
    // calling `widget.allocate(...)` below (which can reenter our
    // tree code).
    let child_ids = tree.children(parent_id).to_vec();
    let plan: Vec<(gtk4::Widget, renderer::Layout)> = child_ids
        .into_iter()
        .filter_map(|cid| {
            let layout = tree.layout(cid)?;
            let widget = tree.view(cid)?;
            Some((widget, layout))
        })
        .collect();

    let parent_ptr = parent_widget.as_ptr();
    for (widget, layout) in plan {
        let parent_match = widget
            .parent()
            .map(|p| p.as_ptr() == parent_ptr)
            .unwrap_or(false);
        if !parent_match {
            continue;
        }
        let transform = gtk4::gsk::Transform::new().translate(
            &gtk4::graphene::Point::new(layout.location.x, layout.location.y),
        );
        let w = layout.size.width.max(0.0).round() as i32;
        let h = layout.size.height.max(0.0).round() as i32;
        widget.allocate(w, h, -1, Some(transform));
    }
}
