//! AppKit-flavoured app mounting for macOS.
//!
//! The web-target equivalent is `mount_to_body` / `mount_to`,
//! which attach the built state to a DOM element. On macOS the
//! equivalent is: build the user's view tree, open its NSWindows,
//! then run the AppKit run loop until the app terminates.
//!
//! ## API shape
//!
//! Every mount entry point returns an [`AppHandle`]. The handle
//! owns the app-lifetime state (root reactive [`Owner`], built
//! view [`State`](renderer::view::Render::State), shared
//! `NSApplication`, and the AppDelegate). The user's `main`
//! binds the handle and calls [`AppHandle::run`] to enter the
//! run loop:
//!
//! ```ignore
//! fn main() {
//!     mount_to_window("Counter", (320.0, 200.0), || view!{ <Counter /> })
//!         .run();
//! }
//! ```
//!
//! When the AppKit run loop returns (Cmd-Q, last-window-close,
//! or programmatic `quit()`), `run()` returns and the handle
//! drops in declared field order: View state → reactive Owner →
//! NSApplication retain → AppDelegate retain. AppKit teardown
//! and reactive cleanup both happen before the process exits,
//! rather than being skipped via `mem::forget`.
//!
//! ### Three entry points
//!
//!  - [`run`] — general purpose. Takes any renderer [`Render`] value
//!    (often a tuple of `Window`s for multi-window apps).
//!  - [`mount_to_window`] — sugar for the common single-window case.
//!  - [`mount_to_split_window`] — opens a window whose content view
//!    controller is an `NSSplitViewController`. The closure returns
//!    a `<split_view>` with one or more `<split_pane>` children.
//!
//! ### Forgetting to call `run`
//!
//! [`AppHandle`] is `#[must_use]`. Dropping the handle without
//! calling `run()` immediately tears the app state down — no
//! window stays visible, the process either exits (if `main`
//! returns) or continues with the AppKit subsystem in an
//! initialised-but-not-running state.

use cocoa_dom::{
    app::{init_app, run_loop, AppDelegate},
    MainThreadMarker,
};
use objc2::rc::Retained;
use objc2_app_kit::NSApplication;
use reactive_graph::owner::Owner;
use crate::{
    cocoa::{
        split::{IntoSplitView, SplitPaneList},
        window::window,
    },
    Dom,
};
use renderer::view::Render;

#[allow(unused_imports)]
use Dom as _ResolveDom;

/// Owns everything the mounted app needs to keep alive for its
/// run-loop lifetime: the user-built view state (type-erased), the
/// root reactive [`Owner`], the shared `NSApplication`, and the
/// AppDelegate.
///
/// Returned by [`run`], [`mount`], [`mount_to_window`], and
/// [`mount_to_split_window`]. Call [`AppHandle::run`] to enter the
/// AppKit run loop; when that returns, the handle's `Drop` runs in
/// field-declared order (state → owner → app → delegate), so
/// reactive cleanup happens BEFORE the process exits.
///
/// Marked `#[must_use]` — dropping without calling `run()`
/// immediately tears the app state down; that's usually a bug.
#[must_use = "AppHandle owns the NSApplication, root reactive Owner, \
              and built view state. Call `.run()` to enter the AppKit \
              run loop, or explicitly drop the handle to tear \
              everything down without showing the app."]
pub struct AppHandle {
    // Field-declared order matters for Drop. State drops first so
    // window-state Drops fire while reactive_graph is still alive;
    // Owner second so the arena can be cleaned without dangling
    // references. App and delegate last; the explicit
    // `setDelegate(None)` in `AppHandle::drop` nils the
    // NSApplication's weak slot before the delegate Retained
    // releases.
    //
    // `allow(dead_code)`: every field is read indirectly via Drop
    // (or via `app` for `run_loop`). The compiler can't see that.
    #[allow(dead_code)]
    state: Box<dyn std::any::Any>,
    #[allow(dead_code)]
    owner: Owner,
    app: Retained<NSApplication>,
    #[allow(dead_code)]
    delegate: Retained<AppDelegate>,
}

impl AppHandle {
    /// Enter the AppKit run loop. Blocks until the app terminates
    /// (Cmd-Q, last-window-close per `set_quit_on_last_window_close`,
    /// or `cocoa_dom::app::quit()`).
    ///
    /// When the run loop returns, `self` drops here — see the
    /// `AppHandle` doc for teardown order.
    pub fn run(self) {
        run_loop(&self.app);
        // self drops at end of scope.
    }
}

impl Drop for AppHandle {
    fn drop(&mut self) {
        // Nil out the app's weak delegate slot before our
        // `Retained<AppDelegate>` releases — same pattern as
        // `OpenedWindow::Drop` / `Toolbar::Drop`. NSApplication
        // is the only place that still references our delegate at
        // this point; setting it to None ensures no late event
        // dispatch lands in a freed delegate.
        if MainThreadMarker::new().is_some() {
            self.app.setDelegate(None);
        }
        // Field drops fire after this in declared order: state
        // (View state), then owner (reactive Owner cleanup),
        // then app (Retained<NSApplication>, harmless — singleton),
        // then delegate (Retained<AppDelegate>, dealloc fires).
    }
}

/// Build an AppKit application whose root view is constructed by
/// `f`. Returns an [`AppHandle`] — call [`AppHandle::run`] to enter
/// the AppKit run loop.
///
/// `f` is invoked once on the main thread inside a fresh reactive
/// [`Owner`] scope. It should return any renderer [`Render`] value —
/// typically one or more [`crate::cocoa::window::Window`]s. Building
/// those opens NSWindows and mounts their content.
///
/// ```ignore
/// fn main() {
///     run(|| (
///         window().title("Main").size((640.0, 480.0)).child(/* main */),
///         window().title("Inspector").size((280.0, 600.0)).child(/* inspector */),
///     )).run();
/// }
/// ```
pub fn run<F, V>(f: F) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    let mtm = MainThreadMarker::new()
        .expect("leptos_native::mount::run must be called from the main thread");

    let (app, delegate) = init_app(mtm);

    // Reactive scope rooted for the app's lifetime. Set as the
    // current owner so the build closure's signals/effects scope
    // under it. Dropped when `AppHandle` drops — after `run()`
    // returns from the AppKit run loop.
    let owner = Owner::new();
    owner.set();

    // Build the user's view tree. For Window children, this opens
    // their NSWindows synchronously. We pass a freshly-created stub
    // tree as the "outer" tree; in practice the top-level view is
    // a `Window` (or tuple of Windows), each of which ignores the
    // outer tree and builds children against its own per-window tree.
    let view = f();
    let state = view.build();

    AppHandle {
        state: Box::new(state),
        owner,
        app,
        delegate,
    }
}

/// One-call entry point: opens a sensible default window
/// (640×480, titled "App") and mounts the view returned by `f`.
/// Sugar over [`mount_to_window`].
pub fn mount<F, V>(f: F) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
{
    mount_to_window("App", (640.0, 480.0), f)
}

/// Open a single macOS window and mount the view returned by `f`
/// as its content. Sugar over [`run`] for the common case.
///
/// `title` is shown in the window's title bar; `size` is the
/// initial content-area size. Accepts `(f64, f64)`, `(i32, i32)`,
/// or any other `Into<WindowSize>`.
///
/// ```ignore
/// fn main() {
///     mount_to_window("Counter", (320.0, 200.0), || view!{ <Counter /> })
///         .run();
/// }
/// ```
pub fn mount_to_window<F, V, S>(title: &str, size: S, f: F) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: Render<Dom> + 'static,
    S: Into<crate::cocoa::window::WindowSize> + 'static,
{
    let title = title.to_owned();
    let size: crate::cocoa::window::WindowSize = size.into();
    run(move || window().title(title).size(size).child(f()))
}

/// Open a window with an `NSSplitViewController` as its content
/// view controller. The closure must return a `SplitView`
/// configured with one or more `<split_pane>`s — each pane runs
/// inside its own Taffy tree, while the AppKit-side split-view
/// owns the panes' outer frames and animates collapse/expand for
/// `Sidebar` / `Inspector` panes.
///
/// ```ignore
/// fn main() {
///     mount_to_split_window("Untitled", (1100.0, 720.0), || {
///         view! {
///             <split_view vertical=true>
///                 <split_pane holding_priority=199.0>
///                     <Canvas />
///                 </split_pane>
///                 <split_pane
///                     behavior=PaneBehavior::Inspector
///                     preferred_thickness=300.0
///                     minimum_thickness=240.0
///                     collapsed=move || sidebar_hidden.get()
///                 >
///                     <Inspector />
///                 </split_pane>
///             </split_view>
///         }
///     }).run();
/// }
/// ```
pub fn mount_to_split_window<F, V, P, S>(
    title: &str,
    size: S,
    f: F,
) -> AppHandle
where
    F: FnOnce() -> V + 'static,
    V: IntoSplitView<P> + 'static,
    P: SplitPaneList + 'static,
    P::State: 'static,
    S: Into<crate::cocoa::window::WindowSize> + 'static,
{
    let mtm = MainThreadMarker::new()
        .expect("mount_to_split_window must be called from the main thread");
    let (app, delegate) = init_app(mtm);

    let owner = Owner::new();
    owner.set();

    let split_view = f().into_split_view();
    let size: crate::cocoa::window::WindowSize = size.into();
    let (opened, state) = split_view.build_and_install(
        title,
        (size.width(), size.height()),
        mtm,
    );

    opened.show(mtm);

    // Bundle (OpenedSplitWindow, per-pane state) into one Box so
    // AppHandle stays type-erased.
    AppHandle {
        state: Box::new((opened, state)),
        owner,
        app,
        delegate,
    }
}
