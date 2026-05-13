//! `window()` builder — Cocoa NSWindow as a tachys [`Render`] type.
//!
//! Each `Window` opens its own NSWindow and owns its own
//! [`TaffyTree`](taffy::TaffyTree). Children built underneath it
//! register into that tree via the `Mountable::mount` cascade in
//! [`super::element`].
//!
//! Multiple `Window`s in the same `mount::run` call (typically as
//! tuple children of an outer wrapper) each get their own NSWindow +
//! tree, fully isolated.
//!
//! `title`, `size`, and `position` are all reactive — pass a closure
//! and the live window updates on signal changes. `on:close` fires
//! when AppKit posts `windowWillClose:`. Hold a [`WindowHandle`]
//! and call `.close()` for programmatic close.
//!
//! All the Cocoa specifics (NSWindow construction, the resize
//! delegate, shutdown) live in `cocoa_dom::window`; this module is
//! just the tachys-side `Render`/`Mountable` glue.

use super::attr::{install, IntoMaybeReactive, MaybeReactive};
use renderer::view::{Mountable, Render};
use crate::Dom;
use cocoa_dom::{
    layout,
    window::{open_window, OpenedWindow},
    Element as CocoaElement, MainThreadMarker,
};
use objc2::rc::Retained;
use objc2_app_kit::NSWindow;
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use reactive_graph::{
    effect::RenderEffect,
    signal::RwSignal,
    traits::{GetUntracked, Set},
};
use send_wrapper::SendWrapper;

#[allow(missing_docs)]
pub struct Window<Children> {
    title: MaybeReactive<String>,
    size: MaybeReactive<WindowSize>,
    position: Option<MaybeReactive<WindowPosition>>,
    on_close: Option<Box<dyn FnMut() + Send + 'static>>,
    handle: Option<WindowHandle>,
    children: Children,
}

/// Window content-area size in points. Implements `From<(f64, f64)>`
/// so `<window size=(640.0, 480.0)>` and `.size((640.0, 480.0))` both
/// work. The two-arg `.size(w, h)` shape was retired when menus
/// landed — the macro can only emit single-value attributes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize(pub f64, pub f64);

impl From<(f64, f64)> for WindowSize {
    fn from((w, h): (f64, f64)) -> Self {
        WindowSize(w, h)
    }
}

// Reactive support: bare `WindowSize` is static; closures returning
// `WindowSize` become Reactive; tuples auto-lift.
impl IntoMaybeReactive<WindowSize> for WindowSize {
    fn into_maybe_reactive(self) -> MaybeReactive<WindowSize> {
        MaybeReactive::Static(self)
    }
}
impl IntoMaybeReactive<WindowSize> for (f64, f64) {
    fn into_maybe_reactive(self) -> MaybeReactive<WindowSize> {
        MaybeReactive::Static(WindowSize(self.0, self.1))
    }
}
impl<F> IntoMaybeReactive<WindowSize> for F
where
    F: Fn() -> WindowSize + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<WindowSize> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

/// Window screen position in points, from the bottom-left of the
/// main screen (AppKit's native origin convention). Implements
/// `From<(f64, f64)>` for `<window position=(0.0, 0.0)>` syntax.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowPosition(pub f64, pub f64);

impl From<(f64, f64)> for WindowPosition {
    fn from((x, y): (f64, f64)) -> Self {
        WindowPosition(x, y)
    }
}

impl IntoMaybeReactive<WindowPosition> for WindowPosition {
    fn into_maybe_reactive(self) -> MaybeReactive<WindowPosition> {
        MaybeReactive::Static(self)
    }
}
impl IntoMaybeReactive<WindowPosition> for (f64, f64) {
    fn into_maybe_reactive(self) -> MaybeReactive<WindowPosition> {
        MaybeReactive::Static(WindowPosition(self.0, self.1))
    }
}
impl<F> IntoMaybeReactive<WindowPosition> for F
where
    F: Fn() -> WindowPosition + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<WindowPosition> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

/// Programmatic handle to a [`Window`]. Created via
/// [`WindowHandle::new`], passed to the builder via `.handle(h)`,
/// then `.close()` from anywhere (e.g. inside a button's
/// `on:click`) to programmatically close the window.
///
/// The handle is `Copy` (its inner state is an `RwSignal`). The
/// underlying `Retained<NSWindow>` is stashed via `SendWrapper` so
/// the signal type stays `Send` (required by reactive_graph), with
/// a runtime main-thread check on access.
#[derive(Debug, Clone, Copy)]
pub struct WindowHandle(RwSignal<Option<SendWrapper<Retained<NSWindow>>>>);

impl WindowHandle {
    /// Create a new, unfilled WindowHandle. The slot is populated
    /// when the matching `<window>` builder runs `Render::build`.
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    /// Programmatically close the underlying NSWindow. No-op if the
    /// handle hasn't been filled yet (the window hasn't built), or
    /// if the window has already closed.
    pub fn close(&self) {
        if let Some(w) = self.0.get_untracked() {
            w.take().close();
        }
    }

    /// Internal: fill the handle. Called by `Window::build` after
    /// `open_window` returns.
    pub(crate) fn load(&self, ns: &Retained<NSWindow>) {
        self.0.set(Some(SendWrapper::new(ns.clone())));
    }
}

impl Default for WindowHandle {
    fn default() -> Self { Self::new() }
}

#[allow(missing_docs)]
pub fn window() -> Window<()> {
    Window {
        title: MaybeReactive::Static(String::from("Untitled")),
        size: MaybeReactive::Static(WindowSize(480.0, 320.0)),
        position: None,
        on_close: None,
        handle: None,
        children: (),
    }
}

impl<Ch> Window<Ch> {
    /// Window title bar text. Reactive — accepts a `&str`,
    /// `String`, or `Fn() -> String` closure.
    pub fn title<V: IntoMaybeReactive<String>>(mut self, t: V) -> Self {
        self.title = t.into_maybe_reactive();
        self
    }

    /// Window content-area size. Reactive — accepts a `(f64, f64)`,
    /// a `WindowSize`, or a closure returning either.
    pub fn size<V: IntoMaybeReactive<WindowSize>>(mut self, size: V) -> Self {
        self.size = size.into_maybe_reactive();
        self
    }

    /// Window screen position (bottom-left origin, AppKit
    /// convention). Reactive. When unset, AppKit picks a default
    /// (typically the cascading-window position the OS assigns).
    pub fn position<V: IntoMaybeReactive<WindowPosition>>(
        mut self,
        p: V,
    ) -> Self {
        self.position = Some(p.into_maybe_reactive());
        self
    }

    /// `on:close=fn` — fires when AppKit posts `windowWillClose:`.
    /// Used to save app state, sync pending changes, etc. The
    /// handler runs on the main thread.
    pub fn on<E, F>(self, _event: E, handler: F) -> Self
    where
        E: crate::event_macos::EventDescriptor<EventType = ()>,
        F: FnMut(()) + Send + 'static,
        Self: SupportsWindowEvent<E>,
    {
        let mut handler = handler;
        Self {
            on_close: Some(Box::new(move || handler(()))),
            ..self
        }
    }

    /// Attach a [`WindowHandle`] to this window. After
    /// `Render::build` runs, the handle's `close()` method will
    /// close the underlying NSWindow.
    pub fn handle(mut self, h: WindowHandle) -> Self {
        self.handle = Some(h);
        self
    }

    pub fn child<NewCh>(self, c: NewCh) -> Window<(Ch, NewCh)> {
        Window {
            title: self.title,
            size: self.size,
            position: self.position,
            on_close: self.on_close,
            handle: self.handle,
            children: (self.children, c),
        }
    }
}

/// Marker trait constraining which events the `Window` builder
/// accepts via `.on(event, fn)`. Currently just `CloseEvent` —
/// add more entries as the event surface grows.
pub trait SupportsWindowEvent<E> {}

/// Window close event marker — `<window on:close=fn>`.
#[allow(non_upper_case_globals)]
pub struct CloseEvent;
#[allow(non_upper_case_globals)]
pub const close: CloseEvent = CloseEvent;

impl crate::event_macos::EventDescriptor for CloseEvent {
    type EventType = ();
    fn into_pending<F>(_handler: F) -> crate::event_macos::PendingHandler
    where
        F: FnMut(()) + Send + 'static,
    {
        // CloseEvent uses the Window-level on_close slot, not the
        // PendingHandler dispatch. This function is only reached if
        // someone tries to use `on(close, ...)` on a non-Window
        // element via the spread-attr path, which is a category
        // error.
        panic!(
            "on:close is only valid on <window> — the handler is \
             stored on the window's on_close slot, not dispatched \
             via PendingHandler. If you're seeing this, on:close \
             ended up on a non-Window element."
        )
    }
}

impl<Ch> SupportsWindowEvent<CloseEvent> for Window<Ch> {}

#[allow(missing_docs)]
pub struct WindowState {
    /// The opened-window bookkeeping: NSWindow, content_root,
    /// TaffyTree, resize+close delegate. Held to keep all of those
    /// alive for as long as the WindowState exists.
    ///
    /// The user's children aren't stored here — they're moved into
    /// the close handler on the WindowDelegate at build time, which
    /// runs `unmount` on them when the NSWindow fires
    /// `windowWillClose:`. This makes per-window cleanup actually
    /// happen on close (handlers + Taffy entries get released)
    /// instead of leaking for the lifetime of the app.
    opened: OpenedWindow,
    /// Reactive effects for title / size / position. Dropped on
    /// unmount which unsubscribes from the reactive graph.
    _effects: Vec<RenderEffect<()>>,
}

impl<Ch: Render<Dom>> Render<Dom> for Window<Ch>
where
    Ch::State: 'static,
{
    type State = WindowState;

    fn build(self) -> Self::State {
        let mtm = MainThreadMarker::new()
            .expect("Window::build must run on the main thread");

        // Pull initial title + size out of the reactive wrappers via
        // an untracked sample — the live install below subscribes
        // properly. We need a concrete title+size to pass to
        // `open_window`.
        let initial_title = match &self.title {
            MaybeReactive::Static(s) => s.clone(),
            MaybeReactive::Reactive(f) => {
                reactive_graph::graph::untrack(|| f())
            }
        };
        let initial_size = match &self.size {
            MaybeReactive::Static(s) => *s,
            MaybeReactive::Reactive(f) => {
                reactive_graph::graph::untrack(|| f())
            }
        };

        let opened = open_window(
            &initial_title,
            (initial_size.0, initial_size.1),
            mtm,
        );

        // Fill the WindowHandle (if any) before mounting so handlers
        // wired by children can call .close() immediately.
        if let Some(handle) = &self.handle {
            handle.load(&opened.nswindow);
        }

        // Reactive title — wire an Effect that calls setTitle on
        // every change.
        let mut effects: Vec<RenderEffect<()>> = Vec::new();
        let nswindow = opened.nswindow.clone();
        if let Some(eff) = install(self.title, move |s: String| {
            nswindow.setTitle(&NSString::from_str(&s));
        }) {
            effects.push(eff);
        }

        // Reactive size — resize the content rect, preserving the
        // window's current top-left so it doesn't jump around.
        let nswindow = opened.nswindow.clone();
        if let Some(eff) = install(self.size, move |sz: WindowSize| {
            let current = nswindow.frame();
            // AppKit's frame origin is bottom-left, so adjust y so
            // the *top* stays where it is.
            let new_y = current.origin.y
                + (current.size.height - sz.1);
            let new_frame = NSRect::new(
                NSPoint::new(current.origin.x, new_y),
                NSSize::new(sz.0, sz.1),
            );
            nswindow.setFrame_display_animate(new_frame, true, false);
        }) {
            effects.push(eff);
        }

        // Reactive position — install only if the user actually set
        // one. AppKit will use its default cascade if we never call
        // setFrameOrigin.
        if let Some(pos_attr) = self.position {
            let nswindow = opened.nswindow.clone();
            if let Some(eff) = install(pos_attr, move |p: WindowPosition| {
                nswindow.setFrameOrigin(NSPoint::new(p.0, p.1));
            }) {
                effects.push(eff);
            }
        }

        // Build the user's view tree, then mount under the content
        // root. The mount cascade propagates the tree to every
        // descendant (each insert_node sees the parent's
        // LayoutHandle and registers the child in the same tree).
        let mut children = self.children.build();
        children.mount(&opened.content_root, None);

        // Initial layout against the contentView's current size.
        let content_size = opened.content_root.ns_view().frame().size;
        layout::compute_layout(opened.content_root.as_node(), content_size);

        // Show the window after layout so we don't flash an empty one.
        opened.show(mtm);

        // Cleanup runs on `windowWillClose:`. Includes:
        //   - children.unmount() — releases Taffy entries, handlers,
        //     and all reactive subscriptions held by the view tree.
        //   - content_root teardown — drops the per-window Taffy tree.
        //   - on_close user handler — fires last, after the cleanup,
        //     in case the user wants to save state without races.
        let content_root_for_cleanup = opened.content_root.clone();
        let mut on_close = self.on_close;
        let _ = opened.delegate.install_close_handler(Box::new(move || {
            children.unmount();
            content_root_for_cleanup.as_node().teardown();
            if let Some(mut cb) = on_close.take() {
                cb();
            }
        }));

        WindowState { opened, _effects: effects }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Reactive attrs reinstall via their effects on each tick.
        // Non-reactive attributes (handle, on_close) are one-shot
        // at build time and aren't re-applied here. This isn't
        // currently triggered by anything in the view tree — Window
        // is always a leaf of `run` / `mount_to_window`, and those
        // entry points only build, never rebuild.
    }
}

impl Mountable<Dom> for WindowState {
    fn unmount(&mut self) {
        // Programmatic close → AppKit fires windowWillClose: →
        // delegate runs the cleanup closure (idempotent: it Option-
        // takes its slot).
        self.opened.close();
    }

    fn mount(
        &mut self,
        _parent: &CocoaElement,
        _marker: Option<&cocoa_dom::Node>,
    ) {
        // Window is its own root; nothing to mount under another
        // Element. The NSWindow was opened in `build()`.
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        // A Window doesn't contribute any elements to its parent's
        // children list — it lives at the OS level.
        Vec::new()
    }
}
