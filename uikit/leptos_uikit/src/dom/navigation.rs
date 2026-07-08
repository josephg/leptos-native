//! UINavigationController integration — push leptos-built screens
//! onto a real navigation stack, with the system back button and
//! interactive edge-swipe pop for free.
//!
//! The scene's window hosts a `UINavigationController` whose root
//! view controller is the app's content root (navigation bar hidden
//! there). [`push`] builds a leptos view into a fresh layout root,
//! wraps it in a `RootViewController` (which owns safe-area +
//! keyboard handling per screen), and pushes it. Pops — back button,
//! edge swipe, or [`pop`] — are observed via the navigation
//! delegate; the pushed view's reactive `Owner` and mounted state
//! are dropped then, and an app-supplied `on_pop` callback runs.

use std::any::Any;
use std::cell::{Cell, RefCell};

use leptos_native::renderer::view::{Mountable, Render};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, ProtocolObject};
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly};
use objc2_foundation::{NSObjectProtocol, NSString};
use objc2_ui_kit::{
    UINavigationController, UINavigationControllerDelegate, UIViewController,
};
use reactive_graph::owner::Owner;

use crate::dom::app::RootViewController;
use crate::dom::layout;
use crate::dom::node::{UikitElem, UikitNodeExt};
use crate::IosBackend;

struct PushedEntry {
    vc: Retained<UIViewController>,
    /// The per-screen layout root the pushed view was mounted under.
    /// Freed explicitly in [`cleanup_popped`] once the screen pops —
    /// nothing else owns its store entry, so without this it leaks one
    /// node per push/pop cycle.
    content_root: UikitElem,
    _state: Box<dyn Any>,
    _owner: Owner,
    on_pop: Option<Box<dyn FnOnce()>>,
}

thread_local! {
    static NAV: RefCell<Option<Retained<UINavigationController>>> =
        const { RefCell::new(None) };
    static STACK: RefCell<Vec<PushedEntry>> = const { RefCell::new(Vec::new()) };
    static DELEGATE: RefCell<Option<Retained<NavDelegate>>> =
        const { RefCell::new(None) };
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

define_class!(
    /// Observes completed navigation transitions so popped screens
    /// (back button or edge swipe) release their leptos state.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    pub struct NavDelegate;

    unsafe impl NSObjectProtocol for NavDelegate {}

    unsafe impl UINavigationControllerDelegate for NavDelegate {
        // Bar visibility must change alongside the transition (not
        // after it): the root lays itself out against safeAreaInsets,
        // so hiding the bar only in didShow leaves a bar-height gap
        // during the pop animation that snaps shut at the end.
        #[unsafe(method(navigationController:willShowViewController:animated:))]
        fn nav_will_show(
            &self,
            nav: &UINavigationController,
            vc: &UIViewController,
            animated: bool,
        ) {
            nav.setNavigationBarHidden_animated(!is_pushed(vc), animated);
        }

        // Enforce the final bar state here too: a cancelled
        // interactive swipe fires willShow for the root (bar hidden)
        // but lands back on the pushed screen.
        #[unsafe(method(navigationController:didShowViewController:animated:))]
        fn nav_did_show(
            &self,
            nav: &UINavigationController,
            vc: &UIViewController,
            _animated: bool,
        ) {
            BUSY.set(false);
            nav.setNavigationBarHidden_animated(!is_pushed(vc), false);
            cleanup_popped(nav);
        }
    }
);

fn is_pushed(vc: &UIViewController) -> bool {
    let vc_ptr: *const UIViewController = vc;
    STACK.with_borrow(|stack| {
        stack
            .iter()
            .any(|entry| Retained::as_ptr(&entry.vc) == vc_ptr)
    })
}

impl NavDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

fn cleanup_popped(nav: &UINavigationController) {
    let live = nav.viewControllers();
    let mut popped: Vec<PushedEntry> = Vec::new();
    STACK.with_borrow_mut(|stack| {
        let mut i = 0;
        while i < stack.len() {
            let vc_ptr = Retained::as_ptr(&stack[i].vc);
            let still_live = live
                .iter()
                .any(|vc| Retained::as_ptr(&vc) == vc_ptr);
            if still_live {
                i += 1;
            } else {
                popped.push(stack.remove(i));
            }
        }
    });
    for mut entry in popped {
        if let Some(cb) = entry.on_pop.take() {
            cb();
        }
        // Drop the entry first — that tears down the mounted view
        // (`_state`) and its reactive `Owner` — then free the per-screen
        // layout root the view was mounted under. `content_root` is a
        // `Copy` id, so it survives the drop; `remove()` on the now
        // childless container reclaims its store slot.
        let content_root = entry.content_root;
        drop(entry);
        content_root.remove();
    }
}

/// Wire the scene's navigation controller into this module. Called
/// once from the scene delegate.
pub(crate) fn install(
    nav: &Retained<UINavigationController>,
    mtm: MainThreadMarker,
) {
    let delegate = NavDelegate::new(mtm);
    unsafe {
        nav.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }
    DELEGATE.with_borrow_mut(|slot| *slot = Some(delegate));
    NAV.with_borrow_mut(|slot| *slot = Some(nav.clone()));
}

/// Push a leptos-built screen onto the navigation stack. The system
/// navigation bar appears with a back button labelled `back_label`;
/// the user can pop with the back button or the edge-swipe gesture,
/// at which point `on_pop` runs and the screen's reactive state is
/// released. No-op while a transition is already in flight, or if
/// the navigation controller isn't installed (headless tests).
pub fn push<F, V>(
    title: &str,
    back_label: &str,
    build: F,
    on_pop: impl FnOnce() + 'static,
) where
    F: FnOnce() -> V,
    V: Render<IosBackend>,
    V::State: Mountable<IosBackend> + 'static,
{
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(nav) = NAV.with_borrow(|n| n.clone()) else {
        return;
    };
    if BUSY.get() {
        return;
    }
    BUSY.set(true);

    let content_root = UikitElem::create_container_with(mtm);
    layout::set_flex_direction(content_root, layout::FlexDirection::Column);
    {
        use leptos_native::renderer::attrs::Dim;
        use leptos_native::renderer::setters;
        setters::set_size_width(content_root, Dim::Pct(1.0));
        setters::set_size_height(content_root, Dim::Pct(1.0));
    }

    let owner = Owner::new();
    let state = owner.with(|| {
        let mut state = build().build();
        state.mount(content_root, None);
        state
    });

    if let Some(top) = nav.topViewController() {
        top.navigationItem()
            .setBackButtonTitle(Some(&NSString::from_str(back_label)));
    }

    let vc = RootViewController::new(mtm, content_root);
    vc.setView(Some(&content_root.ui_view()));
    vc.setTitle(Some(&NSString::from_str(title)));
    let vc: Retained<UIViewController> =
        unsafe { Retained::cast_unchecked(vc) };

    if let Some(nav_view) = nav.view() {
        layout::compute_layout(content_root, nav_view.bounds().size);
    }

    STACK.with_borrow_mut(|stack| {
        stack.push(PushedEntry {
            vc: vc.clone(),
            content_root,
            _state: Box::new(state),
            _owner: owner,
            on_pop: Some(Box::new(on_pop)),
        });
    });

    nav.pushViewController_animated(&vc, true);
}

/// Pop the top pushed screen, if any. The delegate callback handles
/// state cleanup exactly as for user-initiated pops.
pub fn pop() {
    let Some(nav) = NAV.with_borrow(|n| n.clone()) else {
        return;
    };
    if STACK.with_borrow(|s| s.is_empty()) {
        return;
    }
    nav.popViewControllerAnimated(true);
}
