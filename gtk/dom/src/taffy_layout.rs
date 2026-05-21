//! `TaffyLayout` — a `gtk::LayoutManager` subclass that delegates
//! measure + allocate to the ambient [`renderer::scene`] node store.
//!
//! Each container element backing a `<vstack>` / `<hstack>` /
//! `<view>` / etc. has one `TaffyLayout` instance attached via
//! `gtk::Widget::set_layout_manager`. Every instance reads the shared
//! per-thread store, so the layout engine sees the whole hierarchy.
//!
//! Only the *root* container's `allocate` actually runs the layout
//! pass; nested instances just look up their direct children's
//! pre-computed frames and call `child.allocate(...)`.

use crate::layout::{AvailableSpace, Dimension, GtkBackend, NodeId, Size};
use glib::subclass::prelude::*;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::RefCell;
use renderer::scene::LayoutBackend;

mod imp {
    use crate::layout::GtkBackend;
    use super::*;

    pub struct TaffyLayout {
        pub node_id: RefCell<Option<NodeId>>,
        /// True for the root container (the window's content_root) —
        /// only it actually runs the layout pass.
        pub is_root: RefCell<bool>,
    }

    impl Default for TaffyLayout {
        fn default() -> Self {
            TaffyLayout {
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
            let Some(node_id) = *self.node_id.borrow() else {
                return (0, 0, -1, -1);
            };

            // Best-effort: probe natural size by running the pass
            // against `MaxContent`. Only on the root.
            if *self.is_root.borrow() {
                GtkBackend::run_layout_pass(
                    node_id,
                    Size {
                        width: AvailableSpace::MaxContent,
                        height: AvailableSpace::MaxContent,
                    },
                );
            }

            if let Some(layout) = GtkBackend::layout(node_id) {
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
            let Some(node_id) = *self.node_id.borrow() else { return };

            if *self.is_root.borrow() {
                // Force the root to fill the available space exactly
                // and run the layout pass.
                let mut style = GtkBackend::style(node_id).unwrap_or_default();
                style.size = Size {
                    width: Dimension::length(width as f32),
                    height: Dimension::length(height as f32),
                };
                GtkBackend::set_style(node_id, style);
                GtkBackend::run_layout_pass(
                    node_id,
                    Size {
                        width: AvailableSpace::Definite(width as f32),
                        height: AvailableSpace::Definite(height as f32),
                    },
                );
            }

            allocate_children(node_id, widget);
        }
    }
}

glib::wrapper! {
    /// `gtk::LayoutManager` subclass that defers measure + allocate
    /// to the shared node store. See module docs for the full picture.
    pub struct TaffyLayout(ObjectSubclass<imp::TaffyLayout>)
        @extends gtk4::LayoutManager;
}

impl TaffyLayout {
    pub fn new(node_id: NodeId, is_root: bool) -> Self {
        let obj: Self = glib::Object::new();
        let imp = obj.imp();
        *imp.node_id.borrow_mut() = Some(node_id);
        *imp.is_root.borrow_mut() = is_root;
        obj
    }
}

/// Look up each direct child of `parent_id` and `allocate` it to its
/// Taffy-computed frame. Snapshots `(widget, layout)` pairs first so
/// no store borrow is held across `widget.allocate(...)` (which can
/// re-enter this LayoutManager for nested containers).
fn allocate_children(parent_id: NodeId, parent_widget: &gtk4::Widget) {
    let plan: Vec<(gtk4::Widget, renderer::Layout)> =
        GtkBackend::children(parent_id)
            .into_iter()
            .filter_map(|cid| {
                let layout = GtkBackend::layout(cid)?;
                let widget = GtkBackend::view(cid)?;
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
