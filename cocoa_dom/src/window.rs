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
    define_class, rc::Retained, runtime::{NSObject, ProtocolObject},
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSWindow, NSWindowDelegate,
    NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};

define_class!(
    /// NSWindowDelegate that re-runs Taffy layout when the window
    /// resizes. Holds the [`Node`] of the window's content_root so it
    /// can locate the right Taffy tree (each Node carries its own
    /// `Rc`-shared LayoutHandle pointing at its window's tree).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Node]
    pub struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            // AppKit has already resized the contentView before
            // calling this; read the new size off our root NSView and
            // recompute against it.
            let root: &Node = self.ivars();
            let new_size = root.ns_view().frame().size;
            layout::compute_layout(root, new_size);
        }
    }
);

impl WindowDelegate {
    /// Create a delegate bound to `root`. The delegate retains a
    /// clone of the Node (cheap — shared NSView retain + Rc bump);
    /// register it on an NSWindow via `setDelegate(...)`.
    pub fn new(root: Node, mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(root);
        unsafe { objc2::msg_send![super(alloc), init] }
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
    let content_root = Element::create_with("view", mtm);
    layout::set_flex_direction(
        content_root.as_node(),
        layout::FlexDirection::Column,
    );
    let tree = layout::new_tree();
    layout::register_in_tree(content_root.as_node(), &tree);
    nswindow.setContentView(Some(content_root.ns_view()));

    // Resize delegate.
    let delegate = WindowDelegate::new(content_root.as_node().clone(), mtm);
    let delegate_proto: &ProtocolObject<dyn NSWindowDelegate> =
        ProtocolObject::from_ref(&*delegate);
    nswindow.setDelegate(Some(delegate_proto));

    OpenedWindow {
        nswindow,
        content_root,
        tree,
        delegate,
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

