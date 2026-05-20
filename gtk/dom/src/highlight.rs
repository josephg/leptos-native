//! DevTools node highlight overlay.
//!
//! Draws the Chrome-style box-model highlight (margin/border/padding/
//! content regions) over a single node when the devtools frontend asks
//! for it (`Overlay.highlightNode`). One `HighlightOverlayWidget` is
//! installed per window's `gtk::Overlay`; the currently-highlighted node
//! is global state set by [`set_highlight`].
//!
//! Mirrors the structure of [`crate::debug_overlay`], but driven by the
//! CDP server rather than the `~` key, and showing only one node.

use crate::layout::{self, NodeId};
use glib::subclass::prelude::*;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;
use std::cell::{Cell, RefCell};

thread_local! {
    /// The node the frontend is hovering, or `None`.
    static HIGHLIGHT: Cell<Option<NodeId>> = const { Cell::new(None) };
    static OVERLAYS: RefCell<Vec<glib::WeakRef<HighlightOverlayWidget>>> =
        const { RefCell::new(Vec::new()) };
}

// Chrome-ish translucent region colors: [r, g, b, a] in 0..=1.
const CONTENT: [f32; 4] = [0.44, 0.66, 0.86, 0.62]; // blue
const PADDING: [f32; 4] = [0.58, 0.77, 0.49, 0.55]; // green
const BORDER: [f32; 4] = [1.0, 0.90, 0.60, 0.60]; // pale yellow
const MARGIN: [f32; 4] = [0.96, 0.70, 0.42, 0.55]; // orange

/// Set (or clear, with `None`) the highlighted node and redraw overlays.
pub fn set_highlight(node: Option<NodeId>) {
    HIGHLIGHT.with(|h| h.set(node));
    OVERLAYS.with(|o| {
        o.borrow_mut().retain(|w| match w.upgrade() {
            Some(widget) => {
                widget.queue_draw();
                true
            }
            None => false,
        })
    });
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct HighlightOverlayWidget {
        pub root_id: RefCell<Option<NodeId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for HighlightOverlayWidget {
        const NAME: &'static str = "LeptosDevtoolsHighlight";
        type Type = super::HighlightOverlayWidget;
        type ParentType = gtk4::Widget;
    }

    impl ObjectImpl for HighlightOverlayWidget {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for HighlightOverlayWidget {
        fn measure(&self, _o: gtk4::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk4::Snapshot) {
            let Some(node) = HIGHLIGHT.with(|h| h.get()) else { return };
            let Some(root_id) = *self.root_id.borrow() else { return };
            // Only the overlay that owns this node's window draws it.
            if renderer::root_of::<layout::GtkBackend>(node) != root_id {
                return;
            }
            let target: gtk4::Widget = self.obj().clone().upcast();
            draw_node(node, snapshot, &target);
        }
    }
}

glib::wrapper! {
    pub struct HighlightOverlayWidget(ObjectSubclass<imp::HighlightOverlayWidget>)
        @extends gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl HighlightOverlayWidget {
    fn new(root_id: NodeId) -> Self {
        let obj: Self = glib::Object::new();
        *obj.imp().root_id.borrow_mut() = Some(root_id);
        obj.set_can_target(false);
        obj.set_can_focus(false);
        obj.set_focusable(false);
        obj.set_hexpand(true);
        obj.set_vexpand(true);
        obj
    }
}

fn fill(snap: &gtk4::Snapshot, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let rect = gtk4::graphene::Rect::new(x, y, w, h);
    snap.append_color(&gtk4::gdk::RGBA::new(c[0], c[1], c[2], c[3]), &rect);
}

/// Draw the four box-model regions for `node`, outer-first so each
/// smaller box paints on top, leaving each region visible as a ring.
fn draw_node(node: NodeId, snap: &gtk4::Snapshot, target: &gtk4::Widget) {
    let Some(view) = layout::view(node) else { return };
    let Some(b) = view.compute_bounds(target) else { return };
    let lb = layout::layout(node).unwrap_or_default();

    let (bx, by, bw, bh) = (b.x(), b.y(), b.width(), b.height());

    // Margin box (outermost).
    fill(
        snap,
        bx - lb.margin.left,
        by - lb.margin.top,
        bw + lb.margin.left + lb.margin.right,
        bh + lb.margin.top + lb.margin.bottom,
        MARGIN,
    );
    // Border box.
    fill(snap, bx, by, bw, bh, BORDER);
    // Padding box (border box inset by border).
    let px = bx + lb.border.left;
    let py = by + lb.border.top;
    let pw = bw - lb.border.left - lb.border.right;
    let ph = bh - lb.border.top - lb.border.bottom;
    fill(snap, px, py, pw, ph, PADDING);
    // Content box (padding box inset by padding).
    fill(
        snap,
        px + lb.padding.left,
        py + lb.padding.top,
        pw - lb.padding.left - lb.padding.right,
        ph - lb.padding.top - lb.padding.bottom,
        CONTENT,
    );
}

/// Add a highlight overlay child to an existing `gtk::Overlay` and
/// register it for redraws.
pub fn add_to(overlay: &gtk4::Overlay, root_id: NodeId) {
    let widget = HighlightOverlayWidget::new(root_id);
    overlay.add_overlay(&widget);
    OVERLAYS.with(|o| o.borrow_mut().push(widget.downgrade()));
}
