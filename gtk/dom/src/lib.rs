//! DOM-shaped façade over GTK4, used by tachys/leptos to render native
//! Linux UIs.
//!
//! See `cocoa_dom` (the macOS sibling) for the architectural pattern
//! this crate mirrors. Like cocoa_dom, gtk_dom presents [`Node`],
//! [`Element`], [`Text`], and [`Placeholder`] types loosely modelled
//! on `web_sys` equivalents, but backed by [`gtk::Widget`] and its
//! subclasses ([`gtk::Button`], [`gtk::Entry`], ...).
//!
//! Higher layers (`tachys`, `leptos`) target this façade rather than
//! `web_sys` when building for Linux.
//!
//! # Threading
//!
//! All public APIs in this crate must be called on the GTK main
//! thread. `gtk::Widget` is `!Send` natively; we wrap it in
//! `SendWrapper` so `Node` is nominally `Send + 'static`, with a
//! runtime panic if accessed off-main. This mirrors the single-
//! threaded model `web_sys` uses in the browser.

#![cfg(target_os = "linux")]

pub mod app;
pub mod node;
pub mod renderer;
pub mod spawner;
pub mod window;

pub use node::{Element, Node, NodeKind, Placeholder, Text};
pub use renderer::{
    ClassList, CssStyleDeclaration, Event, Renderer, TemplateElement,
};

// Re-export the GTK crates downstream consumers will need so they
// don't have to take their own direct dependency on them.
pub use gtk4 as gtk;
pub use {gio, glib};
