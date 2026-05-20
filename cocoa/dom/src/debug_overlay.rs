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

use crate::{flipped_view::FlippedView, layout::CocoaBackend};
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

// TLS allowed under `MEMORY_POLICY.md` §2 "app-scoped pinning"
// carve-out: this is a debug-only feature, the overlays live for
// the app's lifetime, the map never grows beyond one entry per
// active window, and all access paths use `with_borrow`/`try_with`
// so shutdown-order drops are safe.
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
            // Yellow stroke = alignment rect (visible content
            // inside any AppKit-implicit padding like NSButton's
            // focus-ring slack).
            let align = NSColor::colorWithSRGBRed_green_blue_alpha(
                1.0, 0.95, 0.2, 0.85,
            );
            // Translucent orange fill = flex gap between siblings.
            let gap = NSColor::colorWithSRGBRed_green_blue_alpha(
                1.0, 0.6, 0.0, 0.30,
            );
            // Green = leaf's reported text baseline.
            let baseline = NSColor::colorWithSRGBRed_green_blue_alpha(
                0.2, 1.0, 0.4, 0.9,
            );
            let ivars = self.ivars();
            walk(ivars.root_id, NSPoint::ZERO, &mut |cmd| {
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
                    DrawCmd::AlignmentRect(r) => {
                        align.setStroke();
                        let p = NSBezierPath::bezierPathWithRect(r);
                        p.setLineWidth(1.0);
                        p.stroke();
                    }
                    DrawCmd::Gap(r) => {
                        gap.setFill();
                        let p = NSBezierPath::bezierPathWithRect(r);
                        p.fill();
                    }
                    DrawCmd::Baseline { y, x_start, x_end } => {
                        baseline.setStroke();
                        let p = NSBezierPath::bezierPath();
                        p.moveToPoint(NSPoint::new(x_start, y));
                        p.lineToPoint(NSPoint::new(x_end, y));
                        p.setLineWidth(1.0);
                        p.stroke();
                    }
                }
            });
        }
    }
);

enum DrawCmd {
    /// Border-box outline (magenta) — Taffy `Layout::size`.
    Border(NSRect),
    /// Content-box outline (cyan) — border minus per-edge Taffy
    /// padding. Emitted only for nodes with non-zero padding.
    Content(NSRect),
    /// Alignment-rect outline (yellow) — AppKit's
    /// `alignmentRectInsets` carved out of the border box. This is
    /// the "implicit padding" leaf controls add inside their Taffy
    /// frame (e.g. NSButton's focus-ring slack). Emitted only when
    /// the insets are non-zero.
    AlignmentRect(NSRect),
    /// Horizontal line at the leaf's reported text baseline
    /// (`firstBaselineOffsetFromTop`). Drawn green; only emitted
    /// when the view returns a non-zero baseline (i.e. it carries
    /// text).
    Baseline { y: f64, x_start: f64, x_end: f64 },
    /// Gap between two consecutive flex children (translucent
    /// orange fill). Inferred from their layouts: whichever axis
    /// has empty space between sibling rectangles.
    Gap(NSRect),
}

fn walk(
    node_id: NodeId,
    offset: NSPoint,
    f: &mut dyn FnMut(DrawCmd),
) {
    walk_inner(node_id, offset, f, true)
}

fn walk_inner(
    node_id: NodeId,
    offset: NSPoint,
    f: &mut dyn FnMut(DrawCmd),
    is_root: bool,
) {
    // Read the NSView's actual `frame` — not Taffy's stored
    // `Layout::location`/`size`. The two can diverge if anything
    // post-processes layout after Taffy (e.g. the baseline-alignment
    // pass in `apply_layout`). The overlay's job is to show what's
    // actually on screen, not what Taffy intended.
    //
    // Padding still comes from Taffy: it's a styled property, not
    // something inferable from the rendered frame.
    let (loc_x, loc_y, size_w, size_h, pad_t, pad_r, pad_b, pad_l, kids, view) = {
        let layout = renderer::layout::<CocoaBackend>(node_id).unwrap_or_default();
        let kids: Vec<NodeId> = renderer::children::<CocoaBackend>(node_id);
        let view = renderer::get_node_context::<CocoaBackend>(node_id)
            .map(|c| (*c.view).clone());
        let frame = view.as_ref().map(|v| v.frame());
        let (lx, ly, sw, sh) = match frame {
            Some(f) => (f.origin.x, f.origin.y, f.size.width, f.size.height),
            None => (
                layout.location.x as f64,
                layout.location.y as f64,
                layout.size.width as f64,
                layout.size.height as f64,
            ),
        };
        (
            lx,
            ly,
            sw,
            sh,
            layout.padding.top as f64,
            layout.padding.right as f64,
            layout.padding.bottom as f64,
            layout.padding.left as f64,
            kids,
            view,
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
        if let Some(view) = view.as_ref() {
            let insets = view.alignmentRectInsets();
            let it = insets.top as f64;
            let ir = insets.right as f64;
            let ib = insets.bottom as f64;
            let il = insets.left as f64;
            if it > 0.0 || ir > 0.0 || ib > 0.0 || il > 0.0 {
                let align = NSRect::new(
                    NSPoint::new(abs.x + il, abs.y + it),
                    NSSize::new(
                        (size_w - il - ir).max(0.0),
                        (size_h - it - ib).max(0.0),
                    ),
                );
                f(DrawCmd::AlignmentRect(align));
            }
            // Use the port-corrected baseline (font-metric-derived
            // for NSControls; NSButton's system value is wrong).
            // Same source the layout post-pass uses, so the green
            // line matches whatever `align_items: Baseline` aligns
            // by.
            if let Some(bo) = crate::layout::first_baseline_offset(view) {
                if bo > 0.0 && bo < size_h {
                    f(DrawCmd::Baseline {
                        y: abs.y + bo,
                        x_start: abs.x,
                        x_end: abs.x + size_w,
                    });
                }
            }
        }
    }

    // Gaps between consecutive children: inferred from their actual
    // NSView frames (not Taffy layouts; see comment in `walk_inner`
    // above re: post-Taffy mutations).
    if kids.len() >= 2 {
        let frames: Vec<NSRect> = kids
            .iter()
            .map(|id| {
                renderer::get_node_context::<CocoaBackend>(*id)
                    .map(|c| c.view.frame())
                    .unwrap_or_default()
            })
            .collect();
        for pair in frames.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let a_x = abs.x + a.origin.x;
            let a_y = abs.y + a.origin.y;
            let a_r = a_x + a.size.width;
            let a_b = a_y + a.size.height;
            let b_x = abs.x + b.origin.x;
            let b_y = abs.y + b.origin.y;
            let b_r = b_x + b.size.width;
            let b_b = b_y + b.size.height;

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
        walk_inner(child, abs, f, false);
    }
}

impl DebugOverlayView {
    fn new(
        mtm: MainThreadMarker,
        root_id: NodeId,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm)
            .set_ivars(DebugOverlayIvars { root_id });
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
pub fn install(content_root: &FlippedView, root_id: NodeId, mtm: MainThreadMarker) {
    let bounds = content_root.bounds();
    let overlay = DebugOverlayView::new(mtm, root_id);
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
