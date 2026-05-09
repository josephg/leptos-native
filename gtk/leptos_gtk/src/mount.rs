//! GTK app mounting for Linux.
//!
//! Mirrors `leptos_cocoa::mount` in shape. Two entry points:
//!
//!  - [`run`] — general purpose. Takes any tachys `Render` value,
//!    builds it once the GtkApplication has activated, then runs the
//!    GTK main loop. Use this when your closure returns one or more
//!    [`tachys::gtk::Window`]s.
//!
//!  - [`mount_to_window`] — convenience for the single-window case.
//!    Wraps the user's content in a `window()` with the given
//!    application id, title, and size and delegates to [`run`].
//!
//! Both block until the app terminates (last window closed).

use crate::{gtk::window::window, Dom};
use gtk4::prelude::*;
use gtk_dom::app::{init_app, run_loop};
use reactive_graph::owner::Owner;
use renderer::view::Render;
use std::cell::RefCell;
use std::rc::Rc;

/// Run a GTK application whose root view is built by `f`.
///
/// `application_id` is a reverse-DNS string (e.g.
/// `"org.example.Counter"`) used by GTK for single-instance behavior
/// and settings storage.
///
/// `f` is invoked once on the main thread inside a fresh reactive
/// [`Owner`] scope. It should return any tachys `Render` value —
/// typically one or more [`tachys::gtk::Window`]s. Building those
/// opens GtkApplicationWindows and mounts their content. Then the
/// GTK main loop runs until the app terminates.
pub fn run<F, V>(application_id: &str, f: F)
where
    F: FnOnce(&gtk4::Application) -> V + 'static,
    V: Render<Dom> + 'static,
{
    let app = init_app(application_id);

    // The user's `f` needs the gtk::Application to construct windows
    // (GtkApplicationWindow is built from one). gtk::Application
    // emits its `activate` signal once the main loop is up; we set
    // up the reactive scope and build the view tree there.
    //
    // GTK 4 actually wants `connect_activate` to be called with a
    // FnMut + 'static closure. We move `f` in via a RefCell-Option
    // take dance so it runs at most once.
    let f_cell = Rc::new(RefCell::new(Some(f)));
    app.connect_activate(move |app| {
        let Some(f) = f_cell.borrow_mut().take() else {
            return;
        };

        // Reactive scope rooted for the app's lifetime.
        let owner = Owner::new();
        owner.set();
        std::mem::forget(owner);

        let view = f(app);
        let state = view.build();
        std::mem::forget(state);
    });

    run_loop(&app);
}

/// Open a single GTK window and mount the view returned by `f` as
/// its content. Sugar over [`run`] for the common case.
///
/// `application_id` is the reverse-DNS app id (see [`run`]).
/// `title` is shown in the window's title bar; `size` is the
/// initial content-area size in pixels.
pub fn mount_to_window<F, V>(
    application_id: &str,
    title: &str,
    size: (i32, i32),
    f: F,
) where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    let title = title.to_owned();
    run(application_id, move |app| {
        window()
            .application(app.clone())
            .title(title)
            .size(size.0, size.1)
            .child(f())
    });
}
