//! GTK app mounting for Linux.
//!
//! Mirrors `leptos_cocoa::mount` in shape. Every entry point returns
//! an [`AppHandle`]; the user's `main` binds it and calls
//! [`AppHandle::run`] to enter the GTK main loop:
//!
//! ```ignore
//! fn main() {
//!     mount_to_window("org.example.Counter", "Counter", (320, 200),
//!         || view!{ <Counter /> })
//!         .run();
//! }
//! ```
//!
//!  - [`run`] — general purpose. Takes any tachys `Render` value,
//!    builds it once the GtkApplication has activated, then (via
//!    `run()`) runs the GTK main loop. Use this when your closure
//!    returns one or more [`crate::gtk::window::Window`]s.
//!
//!  - [`mount_to_window`] — convenience for the single-window case.
//!    Wraps the user's content in a `window()` with the given
//!    application id, title, and size and delegates to [`run`].
//!
//! ## Lifetime / teardown
//!
//! Unlike cocoa, GTK builds the view inside the `activate` signal,
//! which fires *during* the main loop — so the built view state and
//! the root reactive [`Owner`] can't exist before `run()` is called.
//! They land in the handle's shared slot when `activate` fires; the
//! activate closure and the handle share that slot via `Rc`. When the
//! main loop returns (last window closed, Ctrl/Cmd-Q, or
//! `app.quit()`), the handle drops and tears the built state down in
//! field-declared order (view state → reactive Owner), so reactive
//! cleanup happens before the process exits — rather than being
//! skipped via `mem::forget`.

use crate::{gtk::window::window, GtkDom};
use gtk4::prelude::*;
use crate::dom::app::{init_app, run_loop};
use reactive_graph::owner::Owner;
use leptos_native::renderer::view::Render;
use std::cell::RefCell;
use std::rc::Rc;
use leptos_native::renderer;

/// App-lifetime state built once the GtkApplication activates: the
/// type-erased root view state and the root reactive [`Owner`].
///
/// Field-declared order is load-bearing for `Drop`: `state` drops
/// before `owner`, so window-state Drops fire while `reactive_graph`
/// is still alive.
struct AppState {
    // `allow(dead_code)`: both fields exist purely to be kept alive
    // and then dropped in order; neither is read directly.
    #[allow(dead_code)]
    state: Box<dyn std::any::Any>,
    #[allow(dead_code)]
    owner: Owner,
}

/// Owns everything a mounted GTK app keeps alive for its run-loop
/// lifetime: the [`gtk4::Application`] plus a slot for the built view
/// state + root [`Owner`] (populated when `activate` fires).
///
/// Returned by [`run`], [`mount`], and [`mount_to_window`]. Call
/// [`AppHandle::run`] to enter the GTK main loop; when that returns,
/// the handle's `Drop` tears the built state down (state → owner) so
/// reactive cleanup happens BEFORE the process exits.
///
/// Marked `#[must_use]` — dropping without calling `run()` never
/// enters the main loop, so no window is shown.
#[must_use = "AppHandle owns the GtkApplication and the built view \
              state. Call `.run()` to enter the GTK main loop, or \
              drop the handle to tear everything down without \
              running the app."]
pub struct AppHandle {
    app: gtk4::Application,
    /// Shared with the `activate` closure (which populates it during
    /// the main loop). Torn down explicitly in `Drop`.
    built: Rc<RefCell<Option<AppState>>>,
}

impl AppHandle {
    /// Enter the GTK main loop. Blocks until the app terminates (last
    /// window closed, Ctrl/Cmd-Q, or `app.quit()`).
    ///
    /// When the loop returns, `self` drops here — see the type docs
    /// for teardown order.
    pub fn run(self) {
        run_loop(&self.app);
        // self drops at end of scope.
    }
}

impl Drop for AppHandle {
    fn drop(&mut self) {
        // Explicitly take and drop the built state/owner here. The
        // activate closure still holds an `Rc` clone of the slot (kept
        // alive by the GtkApplication's signal connection), so merely
        // dropping our `Rc` wouldn't release the contents. `take()`
        // drops the `AppState` in field order: state (view-state
        // Drops) then owner (reactive cleanup).
        let _ = self.built.borrow_mut().take();
    }
}

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
pub fn run<F, V>(
    application_id: impl Into<Option<&'static str>>,
    f: F,
) -> AppHandle
where
    F: FnOnce(&gtk4::Application) -> V + 'static,
    V: Render<GtkDom> + 'static,
{
    let id = resolve_app_id(application_id.into());
    let app = init_app(&id);

    // Shared slot the activate closure fills in and the AppHandle
    // tears down. See the module docs for why the build can't happen
    // before the main loop starts.
    let built: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));

    // The user's `f` needs the gtk::Application to construct windows
    // (GtkApplicationWindow is built from one). gtk::Application
    // emits its `activate` signal once the main loop is up; we set
    // up the reactive scope and build the view tree there.
    //
    // GTK 4 actually wants `connect_activate` to be called with a
    // FnMut + 'static closure. We move `f` in via a RefCell-Option
    // take dance so it runs at most once.
    let f_cell = Rc::new(RefCell::new(Some(f)));
    let built_for_activate = built.clone();
    app.connect_activate(move |app| {
        let Some(f) = f_cell.borrow_mut().take() else {
            return;
        };

        // Reactive scope rooted for the app's lifetime. Held alive by
        // the AppHandle (via the shared slot), dropped on teardown
        // rather than leaked.
        let owner = Owner::new();
        owner.set();

        // Start the Chrome DevTools Protocol server if compiled in and
        // requested at runtime (`LEPTOS_DEVTOOLS` env var). The spawner
        // and glib main loop are both live by the time `activate` fires.
        #[cfg(feature = "devtools")]
        crate::devtools::start_from_env();

        let view = f(app);
        // Stub tree for the top-level build; the actual Window child
        // ignores this and builds against its own per-window tree.
        let state = view.build();

        *built_for_activate.borrow_mut() = Some(AppState {
            state: Box::new(state),
            owner,
        });
    });

    AppHandle { app, built }
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
pub fn mount<F, V>(f: F) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: Render<GtkDom> + 'static,
{
    mount_to_window::<_, _, (i32, i32)>(None, "App", (640, 480), f)
}

pub fn mount_to_window<F, V, S>(
    application_id: impl Into<Option<&'static str>>,
    title: &str,
    size: S,
    f: F,
) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: Render<GtkDom> + 'static,
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
    })
}
