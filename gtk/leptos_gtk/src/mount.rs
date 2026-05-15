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

/// Application ID resolution. Returns the user-supplied id if any;
/// otherwise falls back to `local.cargo.<CARGO_PKG_NAME>` so
/// example crates and first-time apps get a working build with
/// no extra configuration.
///
/// For production apps with a real domain, always pass an
/// explicit reverse-DNS id — it's used for single-instance
/// behaviour, settings paths, and desktop integration.
fn resolve_app_id(id: Option<&str>) -> String {
    if let Some(s) = id {
        return s.to_owned();
    }
    let pkg = option_env!("CARGO_PKG_NAME").unwrap_or("app");
    // Reverse-DNS schemes don't allow underscores or uppercase
    // (per GApplication's validation). Coerce safely.
    let safe: String = pkg
        .chars()
        .map(|c| match c {
            'A'..='Z' => c.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '-' => c,
            _ => '-',
        })
        .collect();
    format!("local.cargo.{safe}")
}

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
pub fn run<F, V>(application_id: impl Into<Option<&'static str>>, f: F)
where
    F: FnOnce(&gtk4::Application) -> V + 'static,
    V: Render<Dom> + 'static,
{
    let id = resolve_app_id(application_id.into());
    let app = init_app(&id);

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
/// One-call entry point: opens a sensible default window
/// (640×480, titled "App", with an auto-generated application
/// ID from `CARGO_PKG_NAME`) and mounts the view returned by
/// `f`. Sugar over [`mount_to_window`] for the simplest "I just
/// want to see something on screen" case.
pub fn mount<F, V>(f: F)
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    mount_to_window::<_, _, (i32, i32)>(None, "App", (640, 480), f);
}

pub fn mount_to_window<F, V, S>(
    application_id: impl Into<Option<&'static str>>,
    title: &str,
    size: S,
    f: F,
) where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
    S: Into<renderer::window::WindowSize> + 'static,
{
    let title = title.to_owned();
    let id = application_id.into();
    let size: renderer::window::WindowSize = size.into();
    let (w, h) = size.as_i32_tuple();
    run(id, move |app| {
        window()
            .application(app.clone())
            .title(title)
            .size(w, h)
            .child(f())
    });
}
