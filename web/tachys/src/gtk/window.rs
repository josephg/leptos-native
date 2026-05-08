//! `window()` builder — GTK `GtkApplicationWindow` as a tachys
//! [`Render`] type.
//!
//! Each `Window` opens its own `GtkApplicationWindow`. Children built
//! underneath it mount into the window's content root.
//!
//! All the GTK-specifics (widget construction, setup) live in
//! `gtk_dom::window`; this module is just the tachys-side
//! `Render`/`Mountable` glue.

use crate::view::{Mountable, Render};
use gtk_dom::{
    window::OpenedWindow,
    Element as GtkElement, Node as GtkNode,
};

#[allow(missing_docs)]
pub struct Window<Children> {
    pub(crate) title: String,
    pub(crate) size: (i32, i32),
    pub(crate) children: Children,
}

#[allow(missing_docs)]
pub fn window() -> Window<()> {
    Window {
        title: String::from("Untitled"),
        size: (480, 320),
        children: (),
    }
}

impl<Ch> Window<Ch> {
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
            title: self.title,
            size: self.size,
            children: (self.children, c),
        }
    }
}

/// Hold the window alive. Created by `mount_gtk::run` inside the
/// `connect_activate` callback; the app owns it.
#[allow(missing_docs)]
pub struct WindowState {
    pub opened: OpenedWindow,
    pub content_root: GtkElement,
}

impl<Ch: Render> Render for Window<Ch>
where
    Ch::State: Mountable,
{
    type State = WindowState;

    fn build(self) -> Self::State {
        // Window::build is called by mount_gtk::run() inside
        // connect_activate, where activate has already fired and we
        // can create GtkApplicationWindow instances.

        // We don't have the Application handle here (it's passed to
        // connect_activate). mount_gtk::run() handles this by
        // bypassing Window::build for the initial setup and calling
        // open_window + children.mount() directly.
        //
        // If Window::build is called from user code (e.g. as a child
        // of another component), this panics — multi-window apps
        // will need the Application handle threaded through.
        // For single-window apps built via mount_to_window, this is
        // unreachable.
        panic!(
            "gtk::Window::build is unreachable — use \
             mount_to_window() (see mount_gtk.rs)"
        );
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

impl Mountable for WindowState {
    fn unmount(&mut self) {
        self.opened.close();
    }

    fn mount(
        &mut self,
        _parent: &GtkElement,
        _marker: Option<&GtkNode>,
    ) {
        // Window is its own root.
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElement> {
        Vec::new()
    }
}
