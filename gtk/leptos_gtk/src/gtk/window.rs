//! `window()` builder — GTK ApplicationWindow as a tachys [`Render`]
//! type. Mirrors `leptos_cocoa::cocoa::window`.

use crate::Dom;
use gtk4::prelude::*;
use gtk_dom::{
    window::{open_window, OpenedWindow},
    Element as GtkElement,
};
use renderer::view::{Mountable, Render};

#[allow(missing_docs)]
pub struct Window<Children> {
    application: Option<gtk4::Application>,
    title: String,
    size: (i32, i32),
    children: Children,
}

#[allow(missing_docs)]
pub fn window() -> Window<()> {
    Window {
        application: None,
        title: String::from("Untitled"),
        size: (480, 320),
        children: (),
    }
}

impl<Ch> Window<Ch> {
    /// The `gtk::Application` to attach this window to. Required —
    /// `Window::build` panics if unset (you nearly always go through
    /// `mount::run` / `mount_to_window` which sets it for you).
    pub fn application(mut self, app: gtk4::Application) -> Self {
        self.application = Some(app);
        self
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }

    pub fn size(mut self, w: i32, h: i32) -> Self {
        self.size = (w, h);
        self
    }

    pub fn child<NewCh>(self, c: NewCh) -> Window<(Ch, NewCh)> {
        Window {
            application: self.application,
            title: self.title,
            size: self.size,
            children: (self.children, c),
        }
    }
}

#[allow(missing_docs)]
pub struct WindowState {
    /// Owning the OpenedWindow keeps the GtkApplicationWindow alive
    /// for the duration of the State (i.e. until `unmount`).
    opened: OpenedWindow,
}

impl<Ch: Render<Dom>> Render<Dom> for Window<Ch>
where
    Ch::State: 'static,
{
    type State = WindowState;

    fn build(self, _outer_tree: &renderer::layout::TreeRef<<Dom as renderer::renderer::Renderer>::Backend>) -> Self::State {
        let app = self
            .application
            .expect("Window::build called without application; \
                     use leptos_gtk::mount::run or mount_to_window \
                     so the gtk::Application is supplied");

        let opened = open_window(&app, &self.title, self.size);

        // Build the user's view tree, then mount under the content
        // root. The mount cascade propagates the tree to every
        // descendant.
        let mut children = self.children.build(&opened.tree);
        children.mount(&opened.content_root, None);

        // Show. GTK runs measure/allocate on the next frame, which
        // dispatches through our TaffyLayout.
        opened.show();

        // Window cleanup on close: hook up `connect_close_request`
        // so the user's view tree gets torn down (effects unhooked,
        // Taffy cleared) when the window goes away. This isn't as
        // tidy as cocoa's WindowDelegate `windowWillClose:` because
        // GTK doesn't run the close handler with a guarantee that
        // the OS hasn't already started destroying widgets — we
        // unmount synchronously inside the handler.
        let content_root_for_cleanup = opened.content_root.clone();
        // GTK callbacks are `Fn`, not `FnMut`; share the cleanup
        // state via Rc<RefCell<Option<...>>> so we can take it
        // exactly once.
        let children_slot: std::rc::Rc<
            std::cell::RefCell<Option<Ch::State>>,
        > = std::rc::Rc::new(std::cell::RefCell::new(Some(children)));
        opened.gtk_window.connect_close_request(move |_| {
            if let Some(mut children) = children_slot.borrow_mut().take() {
                children.unmount();
                content_root_for_cleanup.as_node().teardown();
            }
            glib::Propagation::Proceed
        });

        WindowState { opened }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

impl Mountable<Dom> for WindowState {
    fn unmount(&mut self) {
        self.opened.close();
    }

    fn mount(
        &mut self,
        _parent: &GtkElement,
        _marker: Option<&gtk_dom::Node>,
    ) {
        // Window is its own root; nothing to mount.
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElement> {
        Vec::new()
    }
}
