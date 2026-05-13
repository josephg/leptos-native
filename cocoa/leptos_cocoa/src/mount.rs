//! AppKit-flavoured app mounting for macOS.
//!
//! The web-target equivalent is [`crate::mount`]: `mount_to_body`,
//! `mount_to`, etc., which take a closure returning an `IntoView`
//! and attach the resulting state to a DOM element.
//!
//! On macOS we have two entry points:
//!
//!  - [`run`] — general purpose. Takes any tachys `Render` value,
//!    builds it, then runs the AppKit run loop. Use this when your
//!    closure returns one or more [`tachys::cocoa::Window`]s
//!    (multi-window apps build a tuple of them).
//!
//!  - [`mount_to_window`] — convenience for the common single-window
//!    case. Wraps the user's content in a `window()` with the given
//!    title and size and delegates to [`run`].
//!
//! Both block until the app terminates (Cmd-Q or the user closes the
//! last window).

use cocoa_dom::{
    app::{init_app, run_loop},
    MainThreadMarker,
};
use reactive_graph::owner::Owner;
use crate::{
    cocoa::{
        split::{IntoSplitView, SplitPaneList},
        window::window,
    },
    Dom,
};
use renderer::view::Render;

/// Run an AppKit application whose root view is built by `f`.
///
/// `f` is invoked once on the main thread inside a fresh reactive
/// [`Owner`] scope. It should return any tachys `Render` value —
/// typically one or more [`tachys::cocoa::Window`]s. Building those
/// opens NSWindows and mounts their content. Then the AppKit run
/// loop runs until the app terminates.
///
/// Usage (multi-window):
///
/// ```ignore
/// use leptos::prelude::*;
/// use leptos::tachys::cocoa::window::window;
/// run(|| (
///     window().title("Main").size(640.0, 480.0).child(/* main view */),
///     window().title("Inspector").size(280.0, 600.0).child(/* inspector */),
/// ));
/// ```
pub fn run<F, V>(f: F)
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    let mtm = MainThreadMarker::new()
        .expect("leptos::mount::run must be called from the main thread");

    let app = init_app(mtm);

    // Reactive scope rooted for the app's lifetime. Leaking the Owner
    // is intentional: when the AppKit run loop exits, the OS reclaims
    // everything anyway.
    let owner = Owner::new();
    owner.set();
    std::mem::forget(owner);

    // Build the user's view tree. For Window children, this opens
    // their NSWindows synchronously. We leak the State so its
    // contents (NSWindows, Effects, TaffyTrees) outlive this call.
    let view = f();
    let state = view.build();
    std::mem::forget(state);

    run_loop(&app);
}

/// Open a single macOS window and mount the view returned by `f` as
/// its content. Sugar over [`run`] for the common case.
///
/// `title` is shown in the window's title bar; `size` is the initial
/// content-area size in points.
pub fn mount_to_window<F, V>(title: &str, size: (f64, f64), f: F)
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    let title = title.to_owned();
    run(move || window().title(title).size(size.0, size.1).child(f()));
}

/// Open a window with an `NSSplitViewController` as its content
/// view controller. The closure must return a [`SplitView`]
/// configured with one or more `<split_pane>`s — each pane runs
/// inside its own Taffy tree, while the AppKit-side split-view
/// owns the panes' outer frames and animates collapse/expand for
/// `Sidebar` / `Inspector` panes.
///
/// ```ignore
/// mount_to_split_window("Untitled", (1100.0, 720.0), || {
///     view! {
///         <split_view vertical=true>
///             <split_pane holding_priority=199.0>
///                 <Canvas />
///             </split_pane>
///             <split_pane
///                 behavior=PaneBehavior::Inspector
///                 preferred_thickness=300.0
///                 minimum_thickness=240.0
///                 collapsed=move || sidebar_hidden.get()
///             >
///                 <Inspector />
///             </split_pane>
///         </split_view>
///     }
/// });
/// ```
pub fn mount_to_split_window<F, V, P>(title: &str, size: (f64, f64), f: F)
where
    F: FnOnce() -> V + 'static,
    V: IntoSplitView<P> + 'static,
    P: SplitPaneList,
{
    let mtm = MainThreadMarker::new()
        .expect("mount_to_split_window must be called from the main thread");
    let app = init_app(mtm);

    // App-scoped reactive Owner. Same leak-on-purpose convention
    // as `run` — the OS reclaims everything on app exit.
    let owner = Owner::new();
    owner.set();
    std::mem::forget(owner);

    // The closure body might be a `view!{}` macro invocation
    // (which wraps the SplitView in `View<...>`) or a direct
    // builder call. `IntoSplitView` handles both.
    let split_view = f().into_split_view();
    let (opened, state) = split_view.build_and_install(title, size, mtm);

    opened.show(mtm);

    // Leak the per-pane mount state + the opened window. Same
    // convention as the rest of the mount machinery — main-loop
    // lifetime ≈ process lifetime.
    std::mem::forget(state);
    std::mem::forget(opened);

    run_loop(&app);

    // Silence the warning about Dom being unused in this fn body.
    let _: Option<Dom> = None;
}
