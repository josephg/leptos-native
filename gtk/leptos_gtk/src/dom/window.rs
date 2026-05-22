//! Window-level helpers: opening a `GtkApplicationWindow` whose
//! content area runs through our [`TaffyLayout`].
//!
//! Mirrors `cocoa_dom::window` — the higher-level
//! `leptos_gtk::Window` builds the `Render`/`Mountable` orchestration
//! against this and is responsible for mounting children before
//! calling [`OpenedWindow::show`].

use crate::dom::layout::{self, FlexDirection};
use crate::dom::node::{install_taffy_layout_for_container, GtkElem};
use gtk4::prelude::*;
use leptos_native::renderer;

/// Everything the higher layers need to set up a single window: the
/// `GtkApplicationWindow` itself and the content-root [`GtkElem`] (its
/// child). Nodes live in the ambient per-thread store.
pub struct OpenedWindow {
    pub gtk_window: gtk4::ApplicationWindow,
    pub content_root: GtkElem,
}

/// Open a `GtkApplicationWindow` with the given title and content
/// size. Installs a generic container as the window's child, sets up
/// a Taffy tree rooted at it, and attaches a [`TaffyLayout`] so GTK
/// will run our layout code on every measure/allocate cycle.
///
/// Does *not* call `present()` — that happens via
/// [`OpenedWindow::show`] after the caller mounts content.
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

    let content_root = GtkElem::create_vstack();
    layout::set_flex_direction(content_root, FlexDirection::Column);
    // Fill the window: 100% size resolves against the
    // `AvailableSpace::Definite` Taffy receives at compute time.
    // (See cocoa's window.rs for the rationale — the framework
    // root expresses "I cover the window" via its style, instead
    // of relying on compute_layout to overwrite user-set sizes.)
    {
        use leptos_native::renderer::attrs::Dim;
        renderer::setters::set_size_width(content_root, Dim::Pct(1.0));
        renderer::setters::set_size_height(content_root, Dim::Pct(1.0));
    }

    // Install our TaffyLayout as the content root's layout manager
    // (`is_root=true` so its `allocate` runs the layout pass). The
    // relayout scheduler finds this root dynamically by walking up
    // `parent` from any descendant.
    let root_id = content_root.id();
    install_taffy_layout_for_container(
        &content_root.widget(),
        root_id,
        /* is_root */ true,
    );

    // When any overlay feature is on, wrap the content root in a single
    // `gtk::Overlay` and attach each enabled overlay widget to it.
    #[cfg(any(feature = "debug-overlay", feature = "devtools"))]
    {
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&content_root.widget()));
        #[cfg(feature = "debug-overlay")]
        crate::dom::debug_overlay::add_to(&overlay, &gtk_window, root_id);
        #[cfg(feature = "devtools")]
        crate::dom::highlight::add_to(&overlay, &gtk_window, root_id);
        gtk_window.set_child(Some(&overlay));
    }
    #[cfg(not(any(feature = "debug-overlay", feature = "devtools")))]
    {
        gtk_window.set_child(Some(&content_root.widget()));
    }

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
