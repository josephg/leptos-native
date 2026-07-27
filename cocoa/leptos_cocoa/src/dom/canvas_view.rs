//! `CanvasView` — an NSView subclass that renders a retained list of
//! [`DrawCmd`]s and reports mouse input in canvas-local coordinates.
//!
//! Backs the `<canvas>` element. Like [`FlippedView`], it returns
//! `YES` from `isFlipped`, so both the draw commands (rendered in
//! `drawRect:`) and the mouse coordinates (converted from the window
//! via `convertPoint:fromView:nil`) are top-left-origin, y-down —
//! matching every other coordinate the framework hands to user code.
//!
//! Rendering is retained-mode: the current scene (`Vec<DrawCmd>`)
//! lives in an ivar; `drawRect:` replays it with NSBezierPath.
//! [`CanvasView::set_scene`] diffs against the stored scene before
//! invalidating, so a reactive `scene=` closure that re-runs without
//! actually changing the commands doesn't trigger a redraw.
//!
//! Mouse handlers (`mouseDown:` / `mouseDragged:` / `mouseUp:`) are
//! Rust closures stored directly in the view's ivars. Unlike the
//! target/action retains in `NodeHandlers`, there's no ObjC object
//! pointing back at Rust-owned memory here — the closures live *on*
//! the view, so a lingering AppKit retain on the view keeps the
//! closures alive too and can never dispatch into freed memory.
//!
//! [`FlippedView`]: crate::dom::flipped_view::FlippedView

use crate::dom::Color;
use objc2::{
    define_class, msg_send, rc::Retained, runtime::AnyObject, DefinedClass,
    MainThreadOnly,
};
use objc2_app_kit::{
    NSBezierPath, NSEvent, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSLineCapStyle, NSLineJoinStyle,
    NSStringDrawing,
};
use objc2_core_foundation::CGFloat;
use objc2_foundation::{
    NSAttributedStringKey, NSDictionary, NSPoint, NSRect, NSSize, NSString,
};
use std::cell::RefCell;

/// A point in canvas-local coordinates: origin at TOP-LEFT, y
/// increases downward.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CanvasPoint {
    pub x: f64,
    pub y: f64,
}

/// Retained drawing commands. All coordinates are top-left-origin,
/// y-down.
#[derive(Clone, PartialEq, Debug)]
pub enum DrawCmd {
    StrokeRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        width: f64,
        dashed: bool,
    },
    FillRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
    },
    /// Ellipse inscribed in the given rect.
    StrokeEllipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
        width: f64,
        dashed: bool,
    },
    FillEllipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color: Color,
    },
    /// Open polyline through the points, round caps and round joins.
    Polyline {
        points: Vec<(f64, f64)>,
        color: Color,
        width: f64,
    },
    /// `x, y` is the TOP-LEFT of the rendered text; system font at
    /// `size` pt.
    Text {
        x: f64,
        y: f64,
        text: String,
        color: Color,
        size: f64,
    },
}

/// One mouse handler slot. `RefCell` so the ObjC method can borrow
/// mutably; `Option` because handlers are installed after
/// construction (by the `<canvas>` builder's `Render::build`).
type MouseHandler = RefCell<Option<Box<dyn FnMut(CanvasPoint) + 'static>>>;

/// Ivars stored on each CanvasView: the retained scene plus the
/// three mouse-handler slots.
#[derive(Default)]
pub struct CanvasIvars {
    scene: RefCell<Vec<DrawCmd>>,
    on_mouse_down: MouseHandler,
    on_mouse_drag: MouseHandler,
    on_mouse_up: MouseHandler,
}

define_class!(
    /// NSView subclass backing `<canvas>`: flipped (top-left-origin
    /// child/draw coordinates), renders the stored [`DrawCmd`] list
    /// in `drawRect:`, and forwards mouse down/drag/up to stored
    /// Rust closures in canvas-local coordinates.
    #[unsafe(super(objc2_app_kit::NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CanvasIvars]
    pub struct CanvasView;

    impl CanvasView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        /// Accept the click that focuses the window, so drawing into
        /// an unfocused window starts on the first click instead of
        /// swallowing it as "activate only".
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty_rect: NSRect) {
            // Hold the borrow for the whole replay — handlers can't
            // re-enter here (drawRect only runs from the display
            // pass, never from inside a mouse handler).
            let scene = self.ivars().scene.borrow();
            for cmd in scene.iter() {
                draw_cmd(cmd);
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.canvas_point(event);
            self.fire(&self.ivars().on_mouse_down, p);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let p = self.canvas_point(event);
            self.fire(&self.ivars().on_mouse_drag, p);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let p = self.canvas_point(event);
            self.fire(&self.ivars().on_mouse_up, p);
        }
    }
);

impl CanvasView {
    /// Construct a new canvas view with a zero frame (Taffy
    /// overwrites at layout time) and an empty scene.
    pub fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let alloc = Self::alloc(mtm).set_ivars(CanvasIvars::default());
        unsafe { msg_send![super(alloc), initWithFrame: frame] }
    }

    /// Convert an event's window location to canvas-local top-left-
    /// origin coordinates. The view is flipped, so
    /// `convertPoint:fromView:nil` already yields y-down local
    /// coordinates directly.
    fn canvas_point(&self, event: &NSEvent) -> CanvasPoint {
        let local = self.convertPoint_fromView(event.locationInWindow(), None);
        CanvasPoint { x: local.x, y: local.y }
    }

    /// Invoke one of the mouse-handler slots. Skips (rather than
    /// panics) on re-entrance, mirroring `ActionTarget::action_fired`.
    fn fire(&self, slot: &MouseHandler, p: CanvasPoint) {
        let mut slot = match slot.try_borrow_mut() {
            Ok(s) => s,
            Err(_) => {
                #[cfg(debug_assertions)]
                eprintln!("[cocoa_dom] reentrant canvas mouse handler skipped");
                return;
            }
        };
        if let Some(cb) = slot.as_mut() {
            cb(p);
        }
    }

    /// Replace the retained scene. Diff-guarded: an identical command
    /// list (e.g. a reactive closure re-running without a real
    /// change) doesn't invalidate the view.
    pub fn set_scene(&self, cmds: Vec<DrawCmd>) {
        {
            let mut scene = self.ivars().scene.borrow_mut();
            if *scene == cmds {
                return;
            }
            *scene = cmds;
        }
        self.setNeedsDisplay(true);
    }

    /// Snapshot of the retained scene. Test/introspection helper.
    pub fn scene(&self) -> Vec<DrawCmd> {
        self.ivars().scene.borrow().clone()
    }

    /// Install the `mouse_down` handler. **Single handler per
    /// canvas** (mirrors the single-target/action rule on
    /// NSControl); a second install panics rather than silently
    /// replacing the first.
    pub fn set_on_mouse_down(&self, cb: Box<dyn FnMut(CanvasPoint) + 'static>) {
        Self::install(&self.ivars().on_mouse_down, cb, "mouse_down");
    }

    /// Install the `mouse_drag` handler. Single handler per canvas.
    pub fn set_on_mouse_drag(&self, cb: Box<dyn FnMut(CanvasPoint) + 'static>) {
        Self::install(&self.ivars().on_mouse_drag, cb, "mouse_drag");
    }

    /// Install the `mouse_up` handler. Single handler per canvas.
    pub fn set_on_mouse_up(&self, cb: Box<dyn FnMut(CanvasPoint) + 'static>) {
        Self::install(&self.ivars().on_mouse_up, cb, "mouse_up");
    }

    fn install(
        slot: &MouseHandler,
        cb: Box<dyn FnMut(CanvasPoint) + 'static>,
        name: &str,
    ) {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            panic!(
                "on:{name} installed twice on the same <canvas>. Each \
                 canvas has a single {name} handler slot — combine \
                 your handlers into one closure.",
            );
        }
        *slot = Some(cb);
    }

    /// Test-only: dispatch a synthetic mouse-down at `p` through the
    /// same path a real `mouseDown:` takes (minus the NSEvent, which
    /// can't be synthesised without a window).
    #[doc(hidden)]
    pub fn fire_mouse_down_for_test(&self, p: CanvasPoint) {
        self.fire(&self.ivars().on_mouse_down, p);
    }

    /// Test-only: synthetic mouse-drag.
    #[doc(hidden)]
    pub fn fire_mouse_drag_for_test(&self, p: CanvasPoint) {
        self.fire(&self.ivars().on_mouse_drag, p);
    }

    /// Test-only: synthetic mouse-up.
    #[doc(hidden)]
    pub fn fire_mouse_up_for_test(&self, p: CanvasPoint) {
        self.fire(&self.ivars().on_mouse_up, p);
    }
}

/// Dash pattern used for every `dashed: true` stroke: 6pt on, 4pt off.
const DASH_PATTERN: [CGFloat; 2] = [6.0, 4.0];

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// Configure a stroke path's width + optional dash, set the stroke
/// color, and stroke it. Shared by the rect / ellipse / polyline arms.
fn stroke_path(path: &NSBezierPath, color: Color, width: f64, dashed: bool) {
    path.setLineWidth(width);
    if dashed {
        unsafe {
            path.setLineDash_count_phase(
                DASH_PATTERN.as_ptr(),
                DASH_PATTERN.len() as isize,
                0.0,
            );
        }
    }
    color.to_nscolor().setStroke();
    path.stroke();
}

/// Replay one [`DrawCmd`] into the current graphics context. Only
/// called from `drawRect:` (where a context is guaranteed).
fn draw_cmd(cmd: &DrawCmd) {
    match cmd {
        DrawCmd::StrokeRect { x, y, w, h, color, width, dashed } => {
            let path = NSBezierPath::bezierPathWithRect(rect(*x, *y, *w, *h));
            stroke_path(&path, *color, *width, *dashed);
        }
        DrawCmd::FillRect { x, y, w, h, color } => {
            let path = NSBezierPath::bezierPathWithRect(rect(*x, *y, *w, *h));
            color.to_nscolor().setFill();
            path.fill();
        }
        DrawCmd::StrokeEllipse { x, y, w, h, color, width, dashed } => {
            let path =
                NSBezierPath::bezierPathWithOvalInRect(rect(*x, *y, *w, *h));
            stroke_path(&path, *color, *width, *dashed);
        }
        DrawCmd::FillEllipse { x, y, w, h, color } => {
            let path =
                NSBezierPath::bezierPathWithOvalInRect(rect(*x, *y, *w, *h));
            color.to_nscolor().setFill();
            path.fill();
        }
        DrawCmd::Polyline { points, color, width } => {
            // A polyline needs at least a segment; single points and
            // empty lists render nothing (matches SVG <polyline>).
            let Some((first, rest)) = points.split_first() else { return };
            if rest.is_empty() {
                return;
            }
            let path = NSBezierPath::bezierPath();
            path.moveToPoint(NSPoint::new(first.0, first.1));
            for (x, y) in rest {
                path.lineToPoint(NSPoint::new(*x, *y));
            }
            path.setLineCapStyle(NSLineCapStyle::Round);
            path.setLineJoinStyle(NSLineJoinStyle::Round);
            path.setLineWidth(*width);
            color.to_nscolor().setStroke();
            path.stroke();
        }
        DrawCmd::Text { x, y, text, color, size } => {
            let font = NSFont::systemFontOfSize(*size);
            let ns_color = color.to_nscolor();
            let keys: [&NSAttributedStringKey; 2] = unsafe {
                [NSFontAttributeName, NSForegroundColorAttributeName]
            };
            let font_obj: &AnyObject = &font;
            let color_obj: &AnyObject = &ns_color;
            let attrs = NSDictionary::from_slices(
                &keys,
                &[font_obj, color_obj],
            );
            let ns_text = NSString::from_str(text);
            // In a flipped view drawAtPoint's point is the TOP-LEFT
            // of the rendered text — exactly the DrawCmd contract.
            unsafe {
                ns_text.drawAtPoint_withAttributes(
                    NSPoint::new(*x, *y),
                    Some(&attrs),
                );
            }
        }
    }
}
