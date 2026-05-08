//! `FlippedView` — an NSView subclass with `isFlipped == true`.
//!
//! AppKit defaults to a bottom-left origin coordinate system, but every
//! modern layout engine (Taffy included) emits top-left coordinates.
//! Rather than flipping y-values when we apply layout, we install
//! flipped containers everywhere we lay out children. AppKit then
//! interprets subview frames the way the layout engine produced them.
//!
//! Used as the backing view for tags that act as layout containers:
//! `<view>`, `<stack_view>`, and the fallback for unknown tags. Leaf
//! controls (`<button>`, `<label>`, `<text_field>`) still use their
//! own AppKit classes — flippedness only affects how a view interprets
//! its *children*, not how its parent interprets it.

use objc2::{define_class, msg_send, rc::Retained, MainThreadOnly};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect, NSSize};

define_class!(
    /// NSView subclass that returns `YES` from `isFlipped`, giving
    /// children top-left origin coordinates.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct FlippedView;

    impl FlippedView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedView {
    /// Construct a new flipped view with a zero frame. The frame is
    /// expected to be set later by the layout engine.
    pub fn new(mtm: objc2::MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::ZERO, NSSize::new(0.0, 0.0));
        let alloc = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(alloc), initWithFrame: frame] }
    }
}
