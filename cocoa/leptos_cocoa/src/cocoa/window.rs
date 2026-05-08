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
//! All the Cocoa specifics (NSWindow construction, the resize
//! delegate, shutdown) live in `cocoa_dom::window`; this module is
//! just the tachys-side `Render`/`Mountable` glue.

use crate::view::{Mountable, Render};
use cocoa_dom::{
    layout,
    window::{open_window, OpenedWindow},
    Element as CocoaElement, MainThreadMarker,
};

#[allow(missing_docs)]
pub struct Window<Children> {
    title: String,
    size: (f64, f64),
    children: Children,
}

#[allow(missing_docs)]
pub fn window() -> Window<()> {
    Window {
        title: String::from("Untitled"),
        size: (480.0, 320.0),
        children: (),
    }
}

impl<Ch> Window<Ch> {
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    pub fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }

    pub fn child<NewCh>(self, c: NewCh) -> Window<(Ch, NewCh)> {
        Window {
            title: self.title,
            size: self.size,
            children: (self.children, c),
        }
    }
}

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
}

impl<Ch: Render> Render for Window<Ch>
where
    Ch::State: 'static,
{
    type State = WindowState;

    fn build(self) -> Self::State {
        let mtm = MainThreadMarker::new()
            .expect("Window::build must run on the main thread");

        let opened = open_window(&self.title, self.size, mtm);

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

        // Move children + content_root into a close handler that
        // runs when AppKit fires `windowWillClose:`. Closing the
        // window via Cmd-W, the close button, or `nswindow.close()`
        // all funnel through there, so this is the single cleanup
        // point per window. The closure runs on the main thread
        // (delegate callbacks always do), so accessing the SendWrapper
        // inside is fine.
        let content_root_for_cleanup = opened.content_root.clone();
        let _ = opened.delegate.install_close_handler(Box::new(move || {
            children.unmount();
            content_root_for_cleanup.as_node().teardown();
        }));

        WindowState { opened }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Window-level rebuild semantics aren't defined yet. Reactive
        // children inside still re-fire effects normally; only static
        // changes to title/size on rebuild are dropped today.
    }
}

impl Mountable for WindowState {
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

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        // A Window doesn't contribute any elements to its parent's
        // children list — it lives at the OS level.
        Vec::new()
    }
}
