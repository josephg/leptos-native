//! Window-level helpers: opening a `GtkApplicationWindow` and
//! returning a content root that's ready for child mounting.
//!
//! Used by `tachys::gtk::window::Window` (which builds the actual
//! `Render`/`Mountable` orchestration); kept in `gtk_dom` so all
//! the GTK-specifics stay in one crate.

use crate::node::Element;
use gtk4::prelude::*;

/// Everything the higher layers need to set up a single window: the
/// `GtkApplicationWindow` itself and an [`Element`] (a `gtk::Box`
/// installed as the window's child) that the caller mounts content
/// into.
///
/// The window is *not* presented — call [`OpenedWindow::show`] after
/// mounting children so children get their initial layout pass
/// before the window first appears.
pub struct OpenedWindow {
    pub gtk_window: gtk4::ApplicationWindow,
    pub content_root: Element,
}

/// Open a `GtkApplicationWindow` with the given title and content
/// size. Installs a vertical `gtk::Box` as the window's child and
/// returns it as `content_root` for child mounting.
///
/// Does *not* call `present()` — that happens via [`OpenedWindow::show`]
/// after the caller has mounted content.
pub fn open_window(
    app: &gtk4::Application,
    title: &str,
    size: (i32, i32),
) -> OpenedWindow {
    let gtk_window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title(title)
        .default_width(size.0)
        .default_height(size.1)
        .build();

    // Content root: a vertical box. Mirrors cocoa_dom's choice of a
    // FlippedView with `flex_direction: Column` — children stack top
    // to bottom and fill the window's width.
    let content_root = Element::create("vstack");
    gtk_window.set_child(Some(content_root.widget()));

    OpenedWindow {
        gtk_window,
        content_root,
    }
}

impl OpenedWindow {
    /// Make the window visible.
    pub fn show(&self) {
        self.gtk_window.present();
    }

    /// Close the window.
    pub fn close(&self) {
        self.gtk_window.close();
    }
}
