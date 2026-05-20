//! Debug overlay — strokes a 1px outline around every Taffy-tracked
//! node so you can see what flexbox is actually doing.
//!
//! Mirrors `cocoa_dom::debug_overlay`. Behind the `debug-overlay`
//! cargo feature (off by default). Toggle with `~` (shift+backtick on
//! US keyboards). Visibility is global — all open windows toggle
//! together.
//!
//! Implementation:
//!
//! - The window's content_root is wrapped in a `gtk::Overlay` whose
//!   overlay-child is a custom `DebugOverlayWidget`. The widget sets
//!   `can-target=false` so clicks pass through to the real controls.
//! - The widget's `snapshot` walks the thread-local node store from
//!   its root id and strokes each node's bounds (via
//!   `Widget::compute_bounds`).
//! - A `gtk::EventControllerKey` on the window watches for the `~`
//!   key and flips the global `VISIBLE` flag, then asks every
//!   registered overlay to redraw via `queue_draw`.

use crate::layout::{self, NodeId};
use glib::subclass::prelude::*;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};

static VISIBLE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OVERLAYS: RefCell<Vec<glib::WeakRef<DebugOverlayWidget>>> =
        RefCell::new(Vec::new());
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct DebugOverlayWidget {
        pub root_id: RefCell<Option<NodeId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DebugOverlayWidget {
        const NAME: &'static str = "LeptosDebugOverlay";
        type Type = super::DebugOverlayWidget;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for DebugOverlayWidget {
        fn dispose(&self) {
            // Detach from parent so wrapping `gtk::Overlay` doesn't
            // leak when the window closes.
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for DebugOverlayWidget {
        fn measure(
            &self,
            _orientation: gtk4::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            // Don't influence parent sizing — we rely on
            // hexpand/vexpand to fill the overlay.
            (0, 0, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            if !VISIBLE.load(Ordering::Relaxed) {
                return;
            }
            let Some(root_id) = *self.root_id.borrow() else { return };
            let target: gtk4::Widget = self.obj().clone().upcast();
            walk(root_id, snapshot, &target, true);
        }
    }
}

glib::wrapper! {
    pub struct DebugOverlayWidget(ObjectSubclass<imp::DebugOverlayWidget>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl DebugOverlayWidget {
    fn new(root_id: NodeId) -> Self {
        let obj: Self = glib::Object::new();
        {
            let imp = obj.imp();
            *imp.root_id.borrow_mut() = Some(root_id);
        }
        obj.set_can_target(false);
        obj.set_can_focus(false);
        obj.set_focusable(false);
        obj.set_hexpand(true);
        obj.set_vexpand(true);
        obj
    }
}

/// Color helper: build an RGBA from `[r, g, b, a]` floats in 0..=1.
fn rgba(c: [f32; 4]) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::new(c[0], c[1], c[2], c[3])
}

/// Stroke a 1px outline around `rect` in `color`.
fn stroke_rect(snap: &gtk4::Snapshot, rect: gtk4::graphene::Rect, color: [f32; 4]) {
    let rounded = gtk4::gsk::RoundedRect::from_rect(rect, 0.0);
    let c = rgba(color);
    snap.append_border(&rounded, &[1.0, 1.0, 1.0, 1.0], &[c, c, c, c]);
}

fn fill_rect(snap: &gtk4::Snapshot, rect: gtk4::graphene::Rect, color: [f32; 4]) {
    snap.append_color(&rgba(color), &rect);
}

/// Whether Taffy actually consults `first_baseline(node)` during the
/// layout pass. True when either the node's own `align_self` is
/// `Baseline`, or its flex parent's `align_items` is `Baseline`.
fn baseline_in_use(node_id: NodeId) -> bool {
    let style = match layout::style(node_id) {
        Some(s) => s,
        None => return false,
    };
    if style.align_self == Some(renderer::AlignItems::Baseline) {
        return true;
    }
    let Some(parent_id) = layout::parent(node_id) else { return false };
    let parent_style = match layout::style(parent_id) {
        Some(s) => s,
        None => return false,
    };
    parent_style.align_items == Some(renderer::AlignItems::Baseline)
}

fn walk(
    node_id: NodeId,
    snap: &gtk4::Snapshot,
    target: &gtk4::Widget,
    is_root: bool,
) {
    let Some(view) = layout::view(node_id) else { return };
    let kids: Vec<NodeId> = layout::children(node_id);
    let bounds = view.compute_bounds(target);
    let layout_box = layout::layout(node_id).unwrap_or_default();

    if !is_root {
        if let Some(rect) = bounds {
            // Magenta = border box.
            stroke_rect(snap, rect, [1.0, 0.2, 0.5, 0.85]);

            let pt = layout_box.padding.top;
            let pr = layout_box.padding.right;
            let pb = layout_box.padding.bottom;
            let pl = layout_box.padding.left;
            if pt > 0.0 || pr > 0.0 || pb > 0.0 || pl > 0.0 {
                let cx = rect.x() + pl;
                let cy = rect.y() + pt;
                let cw = (rect.width() - pl - pr).max(0.0);
                let ch = (rect.height() - pt - pb).max(0.0);
                let r = gtk4::graphene::Rect::new(cx, cy, cw, ch);
                // Cyan = content box (border minus padding).
                stroke_rect(snap, r, [0.2, 0.7, 1.0, 0.85]);
            }

            // Green = leaf's reported text baseline. Only drawn when
            // baseline alignment is actually in play — otherwise the
            // value isn't being consumed by layout, and GTK's natural
            // baseline (queried at unconstrained height) can be off
            // for widgets whose final allocation differs from their
            // natural size.
            if baseline_in_use(node_id) {
                if let Some(bo) =
                    <crate::layout::GtkBackend as renderer::LayoutBackend>::first_baseline(&view)
                {
                    if bo > 0.0 && bo < rect.height() {
                        let y = rect.y() + bo;
                        let r = gtk4::graphene::Rect::new(rect.x(), y, rect.width(), 1.0);
                        fill_rect(snap, r, [0.2, 1.0, 0.4, 0.9]);
                    }
                }
            }
        }
    }

    // Translucent orange fill = flex gap between siblings.
    if kids.len() >= 2 {
        let frames: Vec<gtk4::graphene::Rect> = kids
            .iter()
            .filter_map(|id| layout::view(*id).and_then(|w| w.compute_bounds(target)))
            .collect();
        for pair in frames.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let a_r = a.x() + a.width();
            let a_b = a.y() + a.height();
            let b_r = b.x() + b.width();
            let b_b = b.y() + b.height();

            // Horizontal (row) gap.
            if b.x() > a_r + 0.5 {
                let top = a.y().max(b.y());
                let bot = a_b.min(b_b);
                if bot > top {
                    let r = gtk4::graphene::Rect::new(
                        a_r,
                        top,
                        b.x() - a_r,
                        bot - top,
                    );
                    fill_rect(snap, r, [1.0, 0.6, 0.0, 0.30]);
                }
            }
            // Vertical (column) gap.
            if b.y() > a_b + 0.5 {
                let left = a.x().max(b.x());
                let right = a_r.min(b_r);
                if right > left {
                    let r = gtk4::graphene::Rect::new(
                        left,
                        a_b,
                        right - left,
                        b.y() - a_b,
                    );
                    fill_rect(snap, r, [1.0, 0.6, 0.0, 0.30]);
                }
            }
        }
    }

    for child in kids {
        walk(child, snap, target, false);
    }
}

/// Ask every registered overlay to redraw on the next frame. Called
/// from `queue_root_resize` so the bounding boxes follow live updates
/// as elements are added, removed, or resized.
pub fn mark_overlays_dirty() {
    OVERLAYS.with(|o| {
        let mut overlays = o.borrow_mut();
        overlays.retain(|w| {
            if let Some(widget) = w.upgrade() {
                widget.queue_draw();
                true
            } else {
                false
            }
        });
    });
}

/// Add the debug overlay widget to an existing `gtk::Overlay` and
/// install the `~`-toggle key controller on `window`.
pub fn add_to(overlay: &gtk4::Overlay, window: &gtk4::ApplicationWindow, root_id: NodeId) {
    let widget = DebugOverlayWidget::new(root_id);
    overlay.add_overlay(&widget);
    OVERLAYS.with(|o| o.borrow_mut().push(widget.downgrade()));

    install_key_controller(window);
}

fn install_key_controller(window: &gtk4::ApplicationWindow) {
    let controller = gtk4::EventControllerKey::new();
    controller.connect_key_pressed(|_, keyval, _, _| {
        if keyval == gtk4::gdk::Key::asciitilde {
            let new = !VISIBLE.load(Ordering::Relaxed);
            VISIBLE.store(new, Ordering::Relaxed);
            mark_overlays_dirty();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(controller);
}
