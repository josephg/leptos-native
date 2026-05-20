//! DOM-shaped façade over GTK4, used by leptos_gtk to render native
//! Linux UIs.
//!
//! Mirrors `cocoa_dom` (the macOS sibling) in shape, with two main
//! deltas:
//!
//!  - GTK signal-handler closures are owned by the widget itself, so
//!    there's no thread-local handler-store ([`event`] is a thin
//!    routing layer instead of cocoa's `HANDLER_STORE`).
//!  - Layout is computed by Taffy through a custom
//!    [`TaffyLayout`](taffy_layout::TaffyLayout) `gtk::LayoutManager`
//!    rather than our own hand-driven `compute_layout` dispatch —
//!    GTK's measure/allocate cycle calls into our layout manager
//!    every time something queues a resize.
//!
//! `leptos_gtk::Dom` is the [`renderer::Renderer`] impl that drives
//! this façade from a Render tree.
//!
//! # Threading
//!
//! All public APIs in this crate must be called on the GTK main
//! thread. `gtk::Widget` is `!Send` natively; `SendWrapper` makes
//! `Node` nominally `Send + 'static`, with a runtime panic if
//! accessed off-main.

#![cfg(feature = "gtk")]

pub mod app;
pub mod color;
#[cfg(feature = "debug-overlay")]
pub mod debug_overlay;
#[cfg(feature = "devtools")]
pub mod devtools;
pub mod event;
#[cfg(feature = "devtools")]
pub mod highlight;
pub mod layout;
pub mod main_thread;
mod make_view;
pub mod menu;
pub mod node;
pub mod renderer;
pub mod spawner;
pub mod taffy_layout;
pub mod window;

pub use color::Color;
pub use main_thread::on_main;

pub use node::{
    Node, WeakElement, WeakNode,
};
pub use renderer::{
    ClassList, CssStyleDeclaration, Event, Renderer, TemplateElement,
};

// Re-export the GTK crates downstream consumers will need so they
// don't have to take their own direct dependency on them.
pub use gtk4 as gtk;
pub use {gio, glib};
