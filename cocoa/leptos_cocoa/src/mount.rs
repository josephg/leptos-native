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
use crate::{cocoa::window::window, Dom};
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
