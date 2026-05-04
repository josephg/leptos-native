//! GTK-flavoured app mounting for Linux.
//!
//! The web-target equivalent is [`crate::mount`]: `mount_to_body`,
//! `mount_to`, etc., which take a closure returning an `IntoView`
//! and attach the resulting state to a DOM element.
//!
//! On Linux we have two entry points:
//!
//!  - [`run`] — opens a GtkApplication, invokes `f` inside a fresh
//!    reactive [`Owner`] scope during the `activate` signal, builds
//!    the view tree, mounts it into a window, and runs the GTK main
//!    loop until the app terminates.
//!
//!  - [`mount_to_window`] — convenience for the common single-window
//!    case. Opens one window with the given title and size, mounts
//!    the view tree, and runs the loop.
//!
//! Both block until the app terminates.

use gtk_dom::gtk::prelude::*;
use reactive_graph::owner::Owner;
use std::cell::RefCell;
use std::rc::Rc;
use tachys::view::{Mountable, Render};

/// Run a GTK application whose root view is built by `f`.
///
/// `f` is invoked once on the GTK main thread inside a fresh reactive
/// [`Owner`] scope, during `GtkApplication::activate`. It should
/// return any tachys `Render` value with a `Window` as the outermost
/// container (the same pattern as the macOS port's `run`).
pub fn run<F, V>(app_id: &str, f: F)
where
    F: FnOnce() -> V + Send + 'static,
    V: Render,
    V::State: 'static,
{
    let app = gtk_dom::app::init_app(app_id);
    let f = Rc::new(RefCell::new(Some(f)));

    app.connect_activate(move |_app| {
        let f = f.borrow_mut().take().expect("gtk_dom: activate fired twice");
        let owner = Owner::new();
        owner.set();
        std::mem::forget(owner);

        let view = f();
        let state = view.build();
        std::mem::forget(state);
    });

    gtk_dom::app::run_loop(&app);
}

/// Open a single GTK window and mount the view returned by `f` as
/// its content. Sugar over [`run`] for the common case.
///
/// `title` is shown in the window's title bar; `size` is the initial
/// content-area size in pixels.
pub fn mount_to_window<F, V>(
    app_id: &str,
    title: &str,
    size: (i32, i32),
    f: F,
) where
    F: FnOnce() -> V + Send + 'static,
    V: Render,
    V::State: Mountable + 'static,
{
    let app = gtk_dom::app::init_app(app_id);
    let title = title.to_owned();
    let f = Rc::new(RefCell::new(Some(f)));

    app.connect_activate(move |app| {
        let f = f.borrow_mut().take().expect("gtk_dom: activate fired twice");

        let owner = Owner::new();
        owner.set();
        std::mem::forget(owner);

        let opened = gtk_dom::window::open_window(app, &title, size);

        let view = f();
        let mut state = view.build();
        state.mount(&opened.content_root, None);
        std::mem::forget(state);

        opened.show();
    });

    gtk_dom::app::run_loop(&app);
}
