//! UIApplication-level setup: AppDelegate, UIApplicationMain entry.
//!
//! iOS uses UIApplication + UIApplicationDelegate. The app entry
//! point is `UIApplicationMain()`, which creates the UIApplication
//! and AppDelegate, then runs the main event loop forever.
//!
//! The user's view-building closure is stored in a global slot
//! before calling `UIApplicationMain`. The AppDelegate creates the
//! UIWindow, then calls the stored closure to build and mount
//! the view tree.

use objc2::{
    define_class, msg_send,
    rc::{Allocated, Retained},
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_ui_kit::{
    UIApplication, UIApplicationDelegate, UIEdgeInsets, UIScreen,
    UIViewController, UIWindow,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use send_wrapper::SendWrapper;
use std::cell::{Cell, RefCell};

// ---------------------------------------------------------------------
// Global slot for the view-building closure
// ---------------------------------------------------------------------

/// The user's view builder, stored before UIApplicationMain is called.
/// The AppDelegate takes it and calls it with the UIWindow and content
/// root after creating them.
type ViewBuilder = Box<dyn FnOnce(&UIWindow, &crate::node::Element)>;

thread_local! {
    static BUILDER: RefCell<Option<ViewBuilder>> = RefCell::new(None);
}

/// Store a view builder to be invoked when the app launches.
/// Called by the mount entry point before `uiapplication_main`.
pub fn store_view_builder(f: impl FnOnce(&UIWindow, &crate::node::Element) + 'static) {
    BUILDER.with_borrow_mut(|slot| {
        *slot = Some(Box::new(f));
    });
}

// ---------------------------------------------------------------------
// AppDelegate
// ---------------------------------------------------------------------

pub struct AppDelegateState {
    pub window: RefCell<Option<Retained<UIWindow>>>,
    pub content_root: RefCell<Option<crate::node::Element>>,
    /// Owns the content root's Taffy tree. The `LayoutHandle`
    /// stored on each registered node clones this `Rc`, so the tree
    /// would actually stay alive as long as any node references it
    /// — but rooting it here too means there's an explicit owner
    /// rather than relying on a `mem::forget`.
    pub tree: RefCell<Option<crate::layout::TreeRef>>,
}

define_class!(
    /// UIApplicationDelegate that creates a UIWindow at launch,
    /// then calls the stored view builder to populate it.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateState]
    pub struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    impl AppDelegate {
        // Called by UIKit when `UIApplicationMain` does
        // `[[AppDelegate alloc] init]`. Without this method, the
        // ObjC-side allocation leaves our Rust ivars uninitialised
        // and the first `self.ivars()` access panics with
        // "tried to access uninitialized instance variable".
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
            let this = this.set_ivars(AppDelegateState {
                window: RefCell::new(None),
                content_root: RefCell::new(None),
                tree: RefCell::new(None),
            });
            unsafe { msg_send![super(this), init] }
        }
    }

    unsafe impl UIApplicationDelegate for AppDelegate {
        #[unsafe(method(application:didFinishLaunchingWithOptions:))]
        fn did_finish_launching(
            &self,
            _application: &UIApplication,
            _options: Option<&NSObject>,
        ) -> bool {
            let mtm = MainThreadMarker::new()
                .expect("didFinishLaunching must run on main thread");

            let _ = crate::spawner::init();

            // Pre-iOS-13 single-window path. Modern apps create
            // the window from `UISceneDelegate.scene(_:willConnectTo:)`,
            // tracked as audit issue 3a — the modern path requires
            // an Info.plist `UIApplicationSceneManifest`. The old
            // path still works on iOS 15–18.
            #[allow(deprecated)]
            let screen_bounds = UIScreen::mainScreen(mtm).bounds();
            #[allow(deprecated)]
            let window = UIWindow::initWithFrame(
                UIWindow::alloc(mtm),
                screen_bounds,
            );
            window.setBackgroundColor(Some(
                &objc2_ui_kit::UIColor::systemBackgroundColor(),
            ));

            // Content root — a vstack filling the window. The tag
            // already implies `flex_direction: Column` (see
            // `Element::create_with`), so no explicit setter call.
            let content_root = crate::node::Element::create_with("vstack", mtm);
            let tree = crate::layout::new_tree();
            crate::layout::register_in_tree(content_root.as_node(), &tree);

            let root_vc = RootViewController::new(mtm, content_root.clone());
            root_vc.setView(Some(content_root.ui_view()));
            window.setRootViewController(Some(&root_vc));

            // Call the user's view builder.
            BUILDER.with_borrow_mut(|slot| {
                if let Some(build) = slot.take() {
                    build(&window, &content_root);
                }
            });

            window.makeKeyAndVisible();

            *self.ivars().window.borrow_mut() = Some(window);
            *self.ivars().content_root.borrow_mut() = Some(content_root);
            *self.ivars().tree.borrow_mut() = Some(tree);

            true
        }
    }
);

impl AppDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(AppDelegateState {
            window: RefCell::new(None),
            content_root: RefCell::new(None),
            tree: RefCell::new(None),
        });
        unsafe { objc2::msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// RootViewController — re-runs Taffy layout on bounds changes and
// applies the view's safeAreaInsets as padding on the content root.
// ---------------------------------------------------------------------

pub struct RootViewControllerState {
    pub content_root: SendWrapper<crate::node::Element>,
    pub last_insets: Cell<UIEdgeInsets>,
}

define_class!(
    /// `UIViewController` subclass that drives the layout pass.
    ///
    /// UIKit calls `viewDidLayoutSubviews` after every bounds change
    /// (rotation, split-view resize on iPad, keyboard show/hide, and
    /// when `safeAreaInsets` settle on first display) — we use it as
    /// the single hook to re-run `compute_layout` against the
    /// current view bounds, and to push the latest `safeAreaInsets`
    /// onto the content root's Taffy padding so children stay clear
    /// of the status bar / notch / home indicator.
    #[unsafe(super(UIViewController))]
    #[thread_kind = MainThreadOnly]
    #[ivars = RootViewControllerState]
    pub struct RootViewController;

    unsafe impl NSObjectProtocol for RootViewController {}

    impl RootViewController {
        #[unsafe(method(viewDidLayoutSubviews))]
        fn view_did_layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidLayoutSubviews] };

            let view = match self.view() {
                Some(v) => v,
                None => return,
            };
            let bounds = view.bounds();
            let insets = view.safeAreaInsets();

            let state = self.ivars();
            let content_root: &crate::node::Element = &state.content_root;

            // Push safeAreaInsets onto the content root's padding,
            // but only when they actually change — applying every
            // tick would dirty the tree on every layout pass.
            let last = state.last_insets.get();
            if last.top != insets.top
                || last.bottom != insets.bottom
                || last.left != insets.left
                || last.right != insets.right
            {
                state.last_insets.set(insets);
                crate::layout::update_style(content_root.as_node(), |s| {
                    s.padding = taffy::Rect {
                        top: taffy::LengthPercentage::length(insets.top as f32),
                        bottom: taffy::LengthPercentage::length(insets.bottom as f32),
                        left: taffy::LengthPercentage::length(insets.left as f32),
                        right: taffy::LengthPercentage::length(insets.right as f32),
                    };
                });
            }

            crate::layout::compute_layout(content_root.as_node(), bounds.size);
        }
    }
);

impl RootViewController {
    pub fn new(
        mtm: MainThreadMarker,
        content_root: crate::node::Element,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(RootViewControllerState {
            content_root: SendWrapper::new(content_root),
            last_insets: Cell::new(UIEdgeInsets {
                top: 0.0,
                bottom: 0.0,
                left: 0.0,
                right: 0.0,
            }),
        });
        unsafe { msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// UIApplicationMain entry
// ---------------------------------------------------------------------

/// Call UIApplicationMain with our AppDelegate class.
/// This function never returns.
///
/// `objc2`'s `define_class!` registers ObjC classes under a *mangled*
/// name (something like `ios_dom_app_AppDelegate$$...`), not the bare
/// Rust struct name — so we have to look up the runtime name via
/// `AppDelegate::class().name()` and pass that to UIApplicationMain.
/// Hard-coding `"AppDelegate"` makes UIKit fail with
/// `NSInternalInconsistencyException` before launch.
///
/// # Safety
/// Must be called on the main thread. A view builder must have
/// been stored via `store_view_builder` before calling this.
pub fn uiapplication_main() -> ! {
    // Force ObjC class registration. With objc2's define_class!,
    // classes are registered lazily on first use. We touch
    // AppDelegate by allocating a throwaway instance so the runtime
    // table is populated before UIApplicationMain reads it.
    {
        let mtm = MainThreadMarker::new().expect("main thread");
        let _delegate = AppDelegate::new(mtm);
        std::mem::forget(_delegate);
    }

    extern "C" {
        fn UIApplicationMain(
            argc: i32,
            argv: *const *const u8,
            principal_class_name: Option<&NSString>,
            delegate_class_name: Option<&NSString>,
        ) -> i32;
    }

    let cls_name_cstr = AppDelegate::class().name();
    let delegate_name = NSString::from_str(&cls_name_cstr.to_string_lossy());

    let ret = unsafe {
        UIApplicationMain(
            0,
            std::ptr::null(),
            None,
            Some(&delegate_name),
        )
    };

    std::process::exit(ret);
}
