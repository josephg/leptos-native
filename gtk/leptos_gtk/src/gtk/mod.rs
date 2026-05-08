//! GTK-flavoured element builders, the Linux analogue of
//! [`crate::cocoa`]. Each tag here corresponds 1:1 to a GTK4 widget
//! class; tag names are snake_case (`view` → `gtk::Box`, `button` →
//! `gtk::Button`, etc.).
//!
//! The element types implement tachys' [`Render`](crate::view::Render)
//! trait, so they compose with the rest of the view-tree machinery
//! (signals, control flow, `mount_to_*`, etc.).

#![cfg(all(target_os = "linux", leptos_native, feature = "reactive_graph"))]
#![allow(missing_docs)]

pub mod attr;
pub mod bind;
pub mod element;
mod render_html_stub;
pub mod window;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal};
pub use element::{
    button, checkbox, hstack, label, pop_up_button, secure_text_field,
    slider, stack_view, text_field, view, vstack,
};
pub use window::{window, Window};
