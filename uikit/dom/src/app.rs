//! UIApplication-level setup: AppDelegate, SceneDelegate,
//! RootViewController, UIApplicationMain entry.
//!
//! iOS 13+ wants window creation to go through a UISceneDelegate.
//! We declare scene support via Info.plist's
//! `UIApplicationSceneManifest`, then return a programmatic
//! `UISceneConfiguration` from
//! `application:configurationForConnectingSceneSession:options:` —
//! that lets us point UIKit at our SceneDelegate's runtime-mangled
//! ObjC class without baking the name into Info.plist.
//!
//! AppDelegate's role is now slim:
//!   1. Initialise the spawner.
//!   2. Hand UIKit a UISceneConfiguration that names SceneDelegate.
//!
//! SceneDelegate's `scene:willConnectToSession:options:` does the
//! actual work: alloc the UIWindow with `init(windowScene:)`, set
//! up the content root + Taffy tree + RootViewController, run the
//! user's view-building closure, makeKeyAndVisible.
//!
//! The user's view-building closure is stored in a thread-local
//! before `UIApplicationMain` is called.

use objc2::{
    define_class, msg_send,
    rc::{Allocated, Retained},
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_ui_kit::{
    UIApplication, UIApplicationDelegate, UIEdgeInsets, UIScene,
    UISceneConfiguration, UISceneConnectionOptions, UISceneDelegate,
    UISceneSession, UIViewController, UIWindow, UIWindowScene,
    UIWindowSceneDelegate,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use send_wrapper::SendWrapper;
use std::cell::{Cell, RefCell};

// ---------------------------------------------------------------------
// Global slot for the view-building closure
// ---------------------------------------------------------------------

/// The user's view builder, stored before UIApplicationMain is
/// called. The SceneDelegate takes it on
/// `scene:willConnectToSession:` and calls it with the window and
/// content root after creating them.
type ViewBuilder = Box<dyn FnOnce(&UIWindow, &crate::node::Element, &crate::layout::TreeRef)>;

// TLS allowed under `MEMORY_POLICY.md` §2 "app-scoped pinning"
// carve-out: this is a single-value slot used to hand the view
// builder closure across the `UIApplicationMain` boundary
// (`mount::run` stores it; `SceneDelegate::scene:willConnectToSession:`
// takes it once). After consumption the slot is empty for the
// process lifetime.
thread_local! {
    static BUILDER: RefCell<Option<ViewBuilder>> = RefCell::new(None);
}

/// Store a view builder to be invoked when the first scene connects.
/// Called by the mount entry point before `uiapplication_main`.
pub fn store_view_builder(
    f: impl FnOnce(&UIWindow, &crate::node::Element, &crate::layout::TreeRef) + 'static,
) {
    BUILDER.with_borrow_mut(|slot| {
        *slot = Some(Box::new(f));
    });
}

// ---------------------------------------------------------------------
// AppDelegate — slim. Hands UIKit a programmatic scene config.
// ---------------------------------------------------------------------

define_class!(
    /// `UIApplicationDelegate` that
    ///   1. initialises the main-thread spawner on launch, and
    ///   2. returns a `UISceneConfiguration` naming our
    ///      `SceneDelegate` class so UIKit creates one for the
    ///      app's window scene.
    ///
    /// All real work happens in `SceneDelegate`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    impl AppDelegate {
        // UIKit alloc-inits the AppDelegate via `[Class alloc] init]`.
        // Without an explicit init that sets ivars, `self.ivars()`
        // panics on first access. AppDelegate has zero-sized ivars
        // (unit), but the `set_ivars` call is still required to
        // mark the instance initialised.
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
            let this = this.set_ivars(());
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
            let _ = crate::spawner::init();
            true
        }

        // Programmatic scene configuration: returned to UIKit when
        // the system asks for a config for a connecting scene.
        // Avoids having to bake the runtime-mangled SceneDelegate
        // class name into Info.plist (which is read before our code
        // runs).
        //
        // method_family = none tells objc2 to convert our `Retained<T>`
        // into an autoreleased pointer, since this isn't an
        // init/new/create-family method.
        #[unsafe(method_id(application:configurationForConnectingSceneSession:options:))]
        fn configuration_for_connecting_scene_session(
            &self,
            _application: &UIApplication,
            connecting_scene_session: &UISceneSession,
            _options: &UISceneConnectionOptions,
        ) -> Retained<UISceneConfiguration> {
            let mtm = MainThreadMarker::new()
                .expect("scene config callback runs on main thread");
            let role = connecting_scene_session.role();
            let name = NSString::from_str("Default");
            let config = UISceneConfiguration::initWithName_sessionRole(
                UISceneConfiguration::alloc(mtm),
                Some(&name),
                &role,
            );
            unsafe {
                config.setDelegateClass(Some(SceneDelegate::class()));
            }
            config
        }
    }
);

impl AppDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// SceneDelegate — owns the UIWindow + content root + Taffy tree.
// ---------------------------------------------------------------------

pub struct SceneDelegateState {
    pub window: RefCell<Option<Retained<UIWindow>>>,
    pub content_root: RefCell<Option<crate::node::Element>>,
    /// Owns the content root's Taffy tree. Cloned `Rc`s on each
    /// node's `LayoutHandle` already keep the tree alive, but
    /// rooting it here too means there's an explicit owner.
    pub tree: RefCell<Option<crate::layout::TreeRef>>,
}

define_class!(
    /// `UIWindowSceneDelegate` that creates the UIWindow when the
    /// scene connects, sets up the content root + Taffy tree +
    /// `RootViewController`, and runs the user's stored view
    /// builder closure to mount the view tree.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SceneDelegateState]
    pub struct SceneDelegate;

    unsafe impl NSObjectProtocol for SceneDelegate {}

    impl SceneDelegate {
        // UIKit allocs SceneDelegate via `[Class alloc] init]` once
        // the AppDelegate's `configurationForConnectingSceneSession`
        // names this class. Without an explicit init that calls
        // set_ivars, the first `self.ivars()` panics.
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Option<Retained<Self>> {
            let this = this.set_ivars(SceneDelegateState {
                window: RefCell::new(None),
                content_root: RefCell::new(None),
                tree: RefCell::new(None),
            });
            unsafe { msg_send![super(this), init] }
        }
    }

    // `scene:willConnectToSession:options:` is declared on
    // `UISceneDelegate` (UIWindowSceneDelegate inherits from it),
    // so the override has to live in the UISceneDelegate impl —
    // putting it in UIWindowSceneDelegate makes objc2 emit
    // "method not found" because the selector isn't on that
    // protocol's method list.
    unsafe impl UIWindowSceneDelegate for SceneDelegate {}

    unsafe impl UISceneDelegate for SceneDelegate {
        #[unsafe(method(scene:willConnectToSession:options:))]
        fn scene_will_connect(
            &self,
            scene: &UIScene,
            _session: &UISceneSession,
            _options: &UISceneConnectionOptions,
        ) {
            let mtm = MainThreadMarker::new()
                .expect("scene:willConnectToSession: runs on main thread");

            // The scene we get is a UIScene; downcast to UIWindowScene
            // (which it always is for UIWindowSceneDelegate).
            let any: &objc2::runtime::AnyObject = scene.as_ref();
            let Some(window_scene) =
                any.downcast_ref::<UIWindowScene>()
            else {
                eprintln!(
                    "[ios_dom] expected UIWindowScene, got something else"
                );
                return;
            };

            let window = UIWindow::initWithWindowScene(
                UIWindow::alloc(mtm),
                window_scene,
            );
            window.setBackgroundColor(Some(
                &objc2_ui_kit::UIColor::systemBackgroundColor(),
            ));

            // Content root — a vstack filling the window. The tag
            // already implies `flex_direction: Column`. Build the tree
            // first, then create the content root inside it.
            let tree = crate::layout::new_tree();
            let content_root =
                crate::node::Element::create_container_with(&tree, mtm);
            crate::layout::set_flex_direction(
                content_root.as_node(),
                crate::layout::FlexDirection::Column,
            );
            // Fill the window via 100% size — Taffy resolves against
            // the `AvailableSpace::Definite` passed to compute_layout.
            // See cocoa's window.rs for the rationale; matches the
            // cross-port pattern.
            {
                use renderer::attrs::Dim;
                renderer::setters::set_size_width(
                    content_root.as_node(),
                    Dim::Pct(1.0),
                );
                renderer::setters::set_size_height(
                    content_root.as_node(),
                    Dim::Pct(1.0),
                );
            }
            crate::layout::set_as_root(content_root.as_node(), &tree);

            let root_vc = RootViewController::new(mtm, content_root.clone());
            root_vc.setView(Some(content_root.ui_view()));
            window.setRootViewController(Some(&root_vc));

            BUILDER.with_borrow_mut(|slot| {
                if let Some(build) = slot.take() {
                    build(&window, &content_root, &tree);
                }
            });

            window.makeKeyAndVisible();

            *self.ivars().window.borrow_mut() = Some(window);
            *self.ivars().content_root.borrow_mut() = Some(content_root);
            *self.ivars().tree.borrow_mut() = Some(tree);
        }
    }
);

impl SceneDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(SceneDelegateState {
            window: RefCell::new(None),
            content_root: RefCell::new(None),
            tree: RefCell::new(None),
        });
        unsafe { msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// RootViewController — re-runs Taffy layout on bounds changes and
// applies the view's safeAreaInsets + keyboardLayoutGuide as padding
// on the content root.
// ---------------------------------------------------------------------

pub struct RootViewControllerState {
    pub content_root: SendWrapper<crate::node::Element>,
    pub last_insets: Cell<UIEdgeInsets>,
    /// Extra bottom inset added for the on-screen keyboard, in
    /// view coordinates. Recomputed every layout tick from
    /// `view.keyboardLayoutGuide().layoutFrame()`. Zero when the
    /// keyboard is hidden.
    pub last_keyboard_inset: Cell<f64>,
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

            // Keyboard inset: distance from the keyboard's top edge
            // (per `keyboardLayoutGuide.layoutFrame.origin.y`) to the
            // view's bottom, minus the safe-area bottom we already
            // account for via `insets`.
            //
            // On the very first layout pass UIKit hasn't resolved the
            // keyboardLayoutGuide's constraints yet, so layoutFrame
            // comes back as `CGRect.zero`. Treat that as
            // keyboard-hidden — otherwise `bounds.height - 0` would
            // crush the entire content into a tiny rect at the top.
            // Sanity-check via `size.width`: a resolved guide always
            // has the view's full width.
            let kb_layout = view.keyboardLayoutGuide().layoutFrame();
            let kb_bottom_extra = if kb_layout.size.width <= 0.0 {
                0.0
            } else {
                let raw = bounds.size.height - kb_layout.origin.y;
                (raw - insets.bottom).max(0.0)
            };

            let state = self.ivars();
            let content_root: &crate::node::Element = &state.content_root;

            // Push insets + keyboard inset onto the content root's
            // padding, but only when they actually change — applying
            // every tick would dirty the tree on every layout pass.
            let last = state.last_insets.get();
            let last_kb = state.last_keyboard_inset.get();
            let insets_changed = last.top != insets.top
                || last.bottom != insets.bottom
                || last.left != insets.left
                || last.right != insets.right
                || last_kb != kb_bottom_extra;
            if insets_changed {
                state.last_insets.set(insets);
                state.last_keyboard_inset.set(kb_bottom_extra);
                crate::layout::update_style(content_root.as_node(), |s| {
                    s.padding = taffy::Rect {
                        top: taffy::LengthPercentage::length(insets.top as f32),
                        bottom: taffy::LengthPercentage::length(
                            (insets.bottom + kb_bottom_extra) as f32,
                        ),
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
            last_keyboard_inset: Cell::new(0.0),
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
/// name (e.g. `ios_dom_app_AppDelegate$$...`), not the bare Rust struct
/// name — so we have to look up the runtime name via
/// `AppDelegate::class().name()` and pass that to UIApplicationMain.
/// Hard-coding `"AppDelegate"` makes UIKit fail with
/// `NSInternalInconsistencyException` before launch.
///
/// We also touch `SceneDelegate::class()` here so the runtime has it
/// registered before `application:configurationForConnectingSceneSession:`
/// returns a config naming it.
///
/// # Safety
/// Must be called on the main thread. A view builder must have
/// been stored via `store_view_builder` before calling this.
pub fn uiapplication_main() -> ! {
    {
        let mtm = MainThreadMarker::new().expect("main thread");
        // Force class registration. Without these, the classes are
        // registered lazily only on first method dispatch — which
        // for SceneDelegate may not happen until UIKit is already
        // looking it up.
        let _delegate = AppDelegate::new(mtm);
        std::mem::forget(_delegate);
        let _scene_delegate = SceneDelegate::new(mtm);
        std::mem::forget(_scene_delegate);
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
