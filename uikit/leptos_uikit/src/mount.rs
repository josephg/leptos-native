//! UIKit-flavoured app mounting for iOS.
//!
//! Unlike the macOS port (which exposes a `Window` builder for
//! multi-window apps), iOS has just one entry point: [`run`]. iPhone
//! apps run as a single fullscreen `UIWindow`, and iPad multi-window
//! is scene-based — initiated by the user through system gestures,
//! not declared by the app at launch — so a `mount_to_window` /
//! `Window` builder concept doesn't carry over. If iPad multi-window
//! is added later it'll be a `Scene` builder integrated with
//! `UISceneDelegate`, not a window builder.

use crate::Dom;
use ios_dom::app::{store_view_builder, uiapplication_main};
use reactive_graph::owner::Owner;
use renderer::view::{Mountable, Render};

/// Run a UIKit application whose root view is built by `f`.
///
/// `f` is invoked inside `application:didFinishLaunchingWithOptions:`
/// (on the main thread, inside a fresh reactive [`Owner`] scope).
/// It should return any tachys `Render` value — its state is mounted
/// as the root view of the window.
///
/// This function **never returns**.
pub fn run<F, V>(f: F) -> !
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom>,
    V::State: Mountable<Dom> + 'static,
{
    store_view_builder(move |window, content_root| {
        let owner = Owner::new();
        owner.set();
        std::mem::forget(owner);

        let view = f();
        let mut state = view.build();
        state.mount(content_root, None);
        std::mem::forget(state);

        // Initial layout pass: size to the window's frame so views
        // have non-zero frames before the first display tick. Once
        // `viewDidLayoutSubviews` fires (right after
        // `makeKeyAndVisible`) it re-runs layout with safe-area
        // insets applied; this pass is just to avoid a one-frame
        // flash of unsized content.
        let size = window.frame().size;
        ios_dom::layout::compute_layout(content_root.as_node(), size);
    });

    uiapplication_main()
}
