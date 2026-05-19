//! Window-level helpers: opening an NSWindow, the resize delegate.
//!
//! Used by `tachys::cocoa::window::Window` (which builds the actual
//! `Render`/`Mountable` orchestration); kept in `cocoa_dom` so all
//! the AppKit specifics stay in one crate.

use crate::{
    layout::{self, TreeRef},
    node::{Element, Node},
};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, ProtocolObject},
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSWindow, NSWindowDelegate,
    NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::cell::RefCell;

/// Closure invoked when the window is about to close (NSWindow's
/// `windowWillClose:` notification). Runs at most once — install
/// returns the previous one (if any) and `take()` clears the slot.
type CleanupClosure = Box<dyn FnOnce()>;

/// Backing state for [`WindowDelegate`]. Holds the content root (so
/// the resize handler can read its frame) plus an optional cleanup
/// closure that fires once on `windowWillClose:`.
pub struct WindowDelegateState {
    pub root: Node,
    pub on_close: RefCell<Option<CleanupClosure>>,
}

define_class!(
    /// NSWindowDelegate that re-runs Taffy layout on resize and
    /// fires a Rust cleanup closure on close. The cleanup closure
    /// is installed by the higher-level builder (typically tachys'
    /// `WindowState::build`) once it has the children to unmount.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = WindowDelegateState]
    pub struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            // AppKit has already resized the contentView before
            // calling this; read the new size off our root NSView and
            // recompute against it.
            let new_size = self.ivars().root.ns_view().frame().size;
            layout::compute_layout(&self.ivars().root, new_size);
        }

        #[unsafe(method(windowWillClose:))]
        fn window_will_close(&self, _notification: &NSNotification) {
            // Take + run the cleanup closure exactly once. If
            // `install_close_handler` was never called, this is a
            // no-op.
            if let Some(cb) =
                self.ivars().on_close.borrow_mut().take()
            {
                cb();
            }
        }
    }
);

impl WindowDelegate {
    /// Create a delegate bound to `root`. The delegate retains a
    /// clone of the Node (cheap — shared NSView retain + Rc bump);
    /// register it on an NSWindow via `setDelegate(...)`.
    pub fn new(root: Node, mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(WindowDelegateState {
            root,
            on_close: RefCell::new(None),
        });
        unsafe { msg_send![super(alloc), init] }
    }

    /// Install (or replace) the cleanup closure to run on
    /// `windowWillClose:`. Returns any previously-installed closure
    /// without running it (caller's responsibility to drop).
    pub fn install_close_handler(
        &self,
        cb: CleanupClosure,
    ) -> Option<CleanupClosure> {
        self.ivars().on_close.borrow_mut().replace(cb)
    }
}

/// Everything `tachys::cocoa::window::Window::build` needs to set up
/// a single window: the NSWindow itself, the FlippedView used as its
/// contentView, the new TaffyTree the contentView is rooted in, and
/// the resize delegate (already attached to the window). Caller is
/// responsible for keeping the returned values alive (typically by
/// stashing them in the WindowState struct on the caller side) and
/// for mounting children under `content_root`.
pub struct OpenedWindow {
    pub nswindow: Retained<NSWindow>,
    pub content_root: Element,
    pub tree: TreeRef,
    pub delegate: Retained<WindowDelegate>,
}

/// Open an NSWindow with the given title and content size, install a
/// FlippedView as its contentView, register that view as the root of
/// a fresh TaffyTree, and attach a [`WindowDelegate`] for resize
/// reflows.
///
/// Does NOT call `makeKeyAndOrderFront` or activate the app — the
/// caller does that after mounting children, so initial layout fires
/// before the window is shown.
pub fn open_window(
    title: &str,
    size: (f64, f64),
    mtm: MainThreadMarker,
) -> OpenedWindow {
    let content_rect = NSRect::new(
        // Origin chosen as a reasonable centre-screen default for
        // a 1440x900-ish display; we'll do something smarter (cascading
        // window positions, restored frames) once we have a windowing
        // layer worth speaking of.
        NSPoint::new(200.0, 200.0),
        NSSize::new(size.0, size.1),
    );
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Resizable
        | NSWindowStyleMask::Miniaturizable;
    let nswindow: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            content_rect,
            style,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    nswindow.setTitle(&NSString::from_str(title));

    // Content root: a FlippedView, registered in a fresh tree.
    //
    // We set `flex_direction: Column` on the content_root so its
    // immediate children stretch to fill the window's *width* (via
    // the default `align_items: Stretch` in cross-axis). Without
    // this, Taffy defaults to Row flex direction and content-sized
    // children — the user's outermost container would then size
    // itself to whichever sibling has the widest content, causing
    // text fields and other "stretchy" controls to grow with
    // content (the classic "input field grows with each
    // keystroke" bug).
    //
    // Height is still content-sized; if the user's outer container
    // wants to fill the window vertically too, they add
    // `flex_grow=1` to it.
    let tree = layout::new_tree();
    let content_root = Element::create_with(&tree, "view", mtm);
    layout::set_flex_direction(
        content_root.as_node(),
        layout::FlexDirection::Column,
    );
    // The content_root must fill its NSView's bounds (= window's
    // content area). Express that as 100% size so Taffy resolves it
    // against the `AvailableSpace::Definite` we pass to
    // `compute_layout`. This is what makes the root cover the whole
    // window without `compute_layout` having to overwrite user-set
    // sizes on every pass.
    {
        use renderer::attrs::Dim;
        renderer::setters::set_size_width(content_root.as_node(), Dim::Pct(1.0));
        renderer::setters::set_size_height(content_root.as_node(), Dim::Pct(1.0));
    }
    layout::set_as_root(content_root.as_node(), &tree);
    nswindow.setContentView(Some(content_root.ns_view()));

    #[cfg(feature = "debug-overlay")]
    {
        // content_root.ns_view() is a FlippedView for the "view" tag.
        let view: &objc2_app_kit::NSView = content_root.ns_view();
        let any: &objc2::runtime::AnyObject = view.as_ref();
        let flipped: &crate::flipped_view::FlippedView = any
            .downcast_ref::<crate::flipped_view::FlippedView>()
            .expect(
                "debug-overlay: content_root is not a FlippedView — \
                 view tag handling has diverged",
            );
        crate::debug_overlay::install(flipped, &tree, mtm);
    }

    // Resize / close delegate. Pool-wrap the setDelegate call —
    // see `MEMORY_POLICY.md` §4. NSWindow's delegate is documented
    // `weak`; this is a belt-and-braces measure for consistency
    // with the text-system delegate fix.
    let delegate = WindowDelegate::new(content_root.as_node().clone(), mtm);
    objc2::rc::autoreleasepool(|_| {
        let delegate_proto: &ProtocolObject<dyn NSWindowDelegate> =
            ProtocolObject::from_ref(&*delegate);
        nswindow.setDelegate(Some(delegate_proto));
    });

    OpenedWindow {
        nswindow,
        content_root,
        tree,
        delegate,
    }
}

impl Drop for OpenedWindow {
    fn drop(&mut self) {
        // Nil the window's delegate slot before our
        // `Retained<WindowDelegate>` releases. NSWindow holds the
        // delegate weakly, so this is mostly belt-and-braces — but
        // matches the policy pattern used by
        // `NodeHandlers::Drop` /
        // `ToolbarItemRegistration::Drop` / `MenuItem::Drop`.
        // Note: this only fires if we're on the main thread.
        // Off-main drop is a programmer error; the `Retained`s
        // will release without nil-ing first.
        if MainThreadMarker::new().is_some() {
            self.nswindow.setDelegate(None);
        }
    }
}

impl OpenedWindow {
    /// Make the window key and bring the app forward. Pulled out as
    /// a separate step so callers can do the initial layout pass
    /// *before* the window appears (reduces visible flicker).
    pub fn show(&self, mtm: MainThreadMarker) {
        self.nswindow.makeKeyAndOrderFront(None);
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }

    /// Close the window. Called from teardown.
    pub fn close(&self) {
        self.nswindow.close();
    }
}
