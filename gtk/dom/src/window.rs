//! Window-level helpers: opening a `GtkApplicationWindow` whose
//! content area runs through our [`TaffyLayout`].
//!
//! Mirrors `cocoa_dom::window` — the higher-level
//! `leptos_gtk::Window` builds the `Render`/`Mountable` orchestration
//! against this and is responsible for mounting children before
//! calling [`OpenedWindow::show`].

use crate::layout::{self, FlexDirection, TreeRef};
use crate::node::{install_taffy_layout_for_container, Element};
use gtk4::prelude::*;

/// Everything the higher layers need to set up a single window: the
/// `GtkApplicationWindow` itself, the content-root [`Element`] (its
/// child), and the new Taffy tree the content root was registered as
/// root of.
pub struct OpenedWindow {
    pub gtk_window: gtk4::ApplicationWindow,
    pub content_root: Element,
    pub tree: TreeRef,
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

    // Build a fresh tree, then create the content root inside it.
    let tree = layout::new_tree();
    let content_root = Element::create_vstack(&tree);
    layout::set_flex_direction(content_root.as_node(), FlexDirection::Column);
    // Fill the window: 100% size resolves against the
    // `AvailableSpace::Definite` Taffy receives at compute time.
    // (See cocoa's window.rs for the rationale — the framework
    // root expresses "I cover the window" via its style, instead
    // of relying on compute_layout to overwrite user-set sizes.)
    {
        use renderer::attrs::Dim;
        renderer::setters::set_size_width(content_root.as_node(), Dim::Pct(1.0));
        renderer::setters::set_size_height(content_root.as_node(), Dim::Pct(1.0));
    }

    // Publish the content_root as the tree's root and install our
    // TaffyLayout as its layout manager (`is_root=true` so its
    // `allocate` runs `compute_layout`).
    layout::set_as_root(content_root.as_node(), &tree);
    let root_id = content_root
        .as_node()
        .tree_id()
        .expect("just registered")
        .1;
    install_taffy_layout_for_container(
        content_root.widget(),
        &tree,
        root_id,
        /* is_root */ true,
    );

    #[cfg(feature = "debug-overlay")]
    {
        let overlay = crate::debug_overlay::install(
            &gtk_window,
            content_root.widget(),
            &tree,
            root_id,
        );
        gtk_window.set_child(Some(&overlay));
    }
    #[cfg(not(feature = "debug-overlay"))]
    {
        gtk_window.set_child(Some(content_root.widget()));
    }

    OpenedWindow {
        gtk_window,
        content_root,
        tree,
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
