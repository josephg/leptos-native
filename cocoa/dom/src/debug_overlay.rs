//! Debug overlay — strokes a 1px outline around every Taffy-tracked
//! node so you can see what flexbox is actually doing.
//!
//! Behind the `debug-overlay` cargo feature (off by default). Toggle
//! with `~` (shift+backtick on US keyboards). Visibility is global —
//! all open windows toggle together.
//!
//! Implementation:
//!
//! - Each window installs a transparent `DebugOverlayView` as the
//!   topmost subview of its content root, autoresized to fill it.
//! - The view's `drawRect:` walks the window's `TreeRef` and strokes
//!   each node's absolute frame.
//! - A single app-wide `NSEvent` local monitor watches for the `~`
//!   key and flips the global `VISIBLE` flag, then asks every
//!   registered overlay to redraw.
//!
//! The overlay is not registered in Taffy and its `hitTest:` returns
//! null, so it's transparent to layout and to mouse events.

use crate::{flipped_view::FlippedView, layout::TreeRef};
use block2::RcBlock;
use objc2::{
    class, define_class, msg_send,
    rc::Retained,
    runtime::AnyObject,
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSColor, NSEvent, NSEventMask,
    NSView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};
use taffy::NodeId;

/// Tag value the overlay reports for `-tag`. `apply_layout` reads
/// this to skip the overlay when matching subviews to Taffy children.
/// Picked to be far from any value an example would set
/// (NSControl::tag tops out at i32 in practice).
pub const OVERLAY_TAG: isize = -0x1eaf_debe; // arbitrary sentinel
static VISIBLE: AtomicBool = AtomicBool::new(false);
static MONITOR_INSTALLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static OVERLAYS: RefCell<Vec<Retained<DebugOverlayView>>> =
        const { RefCell::new(Vec::new()) };
    /// Keeps the local-monitor handler block alive for the app's
    /// lifetime. AppKit doesn't retain the block strongly — if the
    /// `RcBlock` drops, the monitor invokes a freed pointer.
    static MONITOR_BLOCK: RefCell<Option<RcBlock<dyn Fn(NonNull<NSEvent>) -> *mut NSEvent>>> =
        const { RefCell::new(None) };
    static MONITOR_TOKEN: RefCell<Option<Retained<AnyObject>>> =
        const { RefCell::new(None) };
}

pub struct DebugOverlayIvars {
    tree: TreeRef,
    root_id: NodeId,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DebugOverlayIvars]
    pub struct DebugOverlayView;

    impl DebugOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool { true }

        /// Sentinel value `apply_layout` filters by so the overlay
        /// doesn't shift Taffy child indices in its subview-walk.
        #[unsafe(method(tag))]
        fn tag(&self) -> isize { OVERLAY_TAG }

        /// Transparent to mouse events — clicks pass through to the
        /// real controls underneath.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            if !VISIBLE.load(Ordering::Relaxed) {
                return;
            }
            // Magenta stroke = border box.
            let border = NSColor::colorWithSRGBRed_green_blue_alpha(
                1.0, 0.2, 0.5, 0.85,
            );
            // Cyan stroke = content box (border minus padding).
            let content = NSColor::colorWithSRGBRed_green_blue_alpha(
                0.2, 0.7, 1.0, 0.85,
            );
            // Translucent orange fill = flex gap between siblings.
            let gap = NSColor::colorWithSRGBRed_green_blue_alpha(
                1.0, 0.6, 0.0, 0.30,
            );
            let ivars = self.ivars();
            walk(&ivars.tree, ivars.root_id, NSPoint::ZERO, &mut |cmd| {
                match cmd {
                    DrawCmd::Border(r) => {
                        border.setStroke();
                        let p = NSBezierPath::bezierPathWithRect(r);
                        p.setLineWidth(1.0);
                        p.stroke();
                    }
                    DrawCmd::Content(r) => {
                        content.setStroke();
                        let p = NSBezierPath::bezierPathWithRect(r);
                        p.setLineWidth(1.0);
                        p.stroke();
                    }
                    DrawCmd::Gap(r) => {
                        gap.setFill();
                        let p = NSBezierPath::bezierPathWithRect(r);
                        p.fill();
                    }
                }
            });
        }
    }
);

enum DrawCmd {
    /// Border-box outline (magenta).
    Border(NSRect),
    /// Content-box outline (cyan) — only emitted for nodes with
    /// non-zero padding on at least one edge.
    Content(NSRect),
    /// Gap between two consecutive flex children (translucent
    /// orange fill). Inferred from their layouts: whichever axis
    /// has empty space between sibling rectangles.
    Gap(NSRect),
}

fn walk(
    tree: &TreeRef,
    node_id: NodeId,
    offset: NSPoint,
    f: &mut dyn FnMut(DrawCmd),
) {
    walk_inner(tree, node_id, offset, f, true)
}

fn walk_inner(
    tree: &TreeRef,
    node_id: NodeId,
    offset: NSPoint,
    f: &mut dyn FnMut(DrawCmd),
    is_root: bool,
) {
    let (loc_x, loc_y, size_w, size_h, pad_t, pad_r, pad_b, pad_l, kids) = {
        let t = tree.tree.borrow();
        let layout = t.layout(node_id).copied().unwrap_or_default();
        let kids: Vec<NodeId> = t.children(node_id).unwrap_or_default();
        (
            layout.location.x as f64,
            layout.location.y as f64,
            layout.size.width as f64,
            layout.size.height as f64,
            layout.padding.top as f64,
            layout.padding.right as f64,
            layout.padding.bottom as f64,
            layout.padding.left as f64,
            kids,
        )
    };
    let abs = NSPoint::new(offset.x + loc_x, offset.y + loc_y);
    if !is_root {
        let border = NSRect::new(abs, NSSize::new(size_w, size_h));
        f(DrawCmd::Border(border));
        if pad_t > 0.0 || pad_r > 0.0 || pad_b > 0.0 || pad_l > 0.0 {
            let content = NSRect::new(
                NSPoint::new(abs.x + pad_l, abs.y + pad_t),
                NSSize::new(
                    (size_w - pad_l - pad_r).max(0.0),
                    (size_h - pad_t - pad_b).max(0.0),
                ),
            );
            f(DrawCmd::Content(content));
        }
    }

    // Gaps between consecutive children: inferred from their
    // layouts (no need to read parent style). For each pair of
    // siblings, find the empty band between them on whichever
    // axis has separation.
    if kids.len() >= 2 {
        let layouts: Vec<_> = {
            let t = tree.tree.borrow();
            kids.iter()
                .map(|id| t.layout(*id).copied().unwrap_or_default())
                .collect()
        };
        for pair in layouts.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let a_x = abs.x + a.location.x as f64;
            let a_y = abs.y + a.location.y as f64;
            let a_r = a_x + a.size.width as f64;
            let a_b = a_y + a.size.height as f64;
            let b_x = abs.x + b.location.x as f64;
            let b_y = abs.y + b.location.y as f64;
            let b_r = b_x + b.size.width as f64;
            let b_b = b_y + b.size.height as f64;

            // Horizontal (row) gap.
            if b_x > a_r + 0.5 {
                let top = a_y.max(b_y);
                let bot = a_b.min(b_b);
                if bot > top {
                    f(DrawCmd::Gap(NSRect::new(
                        NSPoint::new(a_r, top),
                        NSSize::new(b_x - a_r, bot - top),
                    )));
                }
            }
            // Vertical (column) gap.
            if b_y > a_b + 0.5 {
                let left = a_x.max(b_x);
                let right = a_r.min(b_r);
                if right > left {
                    f(DrawCmd::Gap(NSRect::new(
                        NSPoint::new(left, a_b),
                        NSSize::new(right - left, b_y - a_b),
                    )));
                }
            }
        }
    }

    for child in kids {
        walk_inner(tree, child, abs, f, false);
    }
}

impl DebugOverlayView {
    fn new(
        mtm: MainThreadMarker,
        tree: TreeRef,
        root_id: NodeId,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm)
            .set_ivars(DebugOverlayIvars { tree, root_id });
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let this: Retained<Self> =
            unsafe { msg_send![super(alloc), initWithFrame: frame] };
        this.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        OVERLAYS.with(|o| o.borrow_mut().push(this.clone()));
        this
    }
}

/// Ask every registered overlay to redraw on the next display pass.
/// Called from `compute_layout` so the bounding boxes follow live
/// updates as elements are added, removed, or resized.
pub fn mark_overlays_dirty() {
    OVERLAYS.with(|o| {
        for overlay in o.borrow().iter() {
            overlay.setNeedsDisplay(true);
        }
    });
}

/// Install a debug overlay over `content_root`, sized to fill it.
/// Idempotent across windows; each call registers another overlay.
pub fn install(content_root: &FlippedView, tree: &TreeRef, mtm: MainThreadMarker) {
    let root_id = {
        // The content_root's NodeId in `tree` — we look it up via the
        // tree's stored root rather than poking at the `Node`'s
        // LayoutHandle (cocoa_dom::node isn't visible here without
        // a circular dep).
        tree.root.borrow().expect(
            "debug_overlay::install: tree has no root yet",
        )
    };

    let bounds = content_root.bounds();
    let overlay = DebugOverlayView::new(mtm, tree.clone(), root_id);
    overlay.setFrame(bounds);

    // Add as topmost subview of content_root, above all existing
    // children. Subsequent additions of new children stay below the
    // overlay because we use `Above` ordering.
    let parent: &NSView = content_root;
    let overlay_view: &NSView = &overlay;
    parent.addSubview_positioned_relativeTo(
        overlay_view,
        NSWindowOrderingMode::Above,
        None,
    );

    ensure_key_monitor();
}

fn ensure_key_monitor() {
    if MONITOR_INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    let handler = move |event: NonNull<NSEvent>| -> *mut NSEvent {
        let event = unsafe { event.as_ref() };
        let chars = event.charactersIgnoringModifiers();
        if let Some(chars) = chars {
            if chars.to_string() == "~" {
                let new = !VISIBLE.load(Ordering::Relaxed);
                VISIBLE.store(new, Ordering::Relaxed);
                OVERLAYS.with(|o| {
                    for overlay in o.borrow().iter() {
                        overlay.setNeedsDisplay(true);
                    }
                });
                return std::ptr::null_mut(); // consume
            }
        }
        // Pass through — caller expects a borrowed pointer back.
        let ptr: *const NSEvent = event;
        ptr as *mut NSEvent
    };
    let block = RcBlock::new(handler);
    let token: Option<Retained<AnyObject>> = unsafe {
        msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: NSEventMask::KeyDown,
            handler: &*block,
        ]
    };
    MONITOR_BLOCK.with(|b| *b.borrow_mut() = Some(block));
    MONITOR_TOKEN.with(|t| *t.borrow_mut() = token);
}
