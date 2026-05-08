//! Cocoa-flavoured element builders, the macOS analogue of
//! [`crate::html::element`]. Each tag here corresponds 1:1 to an
//! AppKit class; tag names drop the `NS` prefix and are snake_case
//! (`view` → NSView, `button` → NSButton, `stack_view` → flipped
//! NSView with column flex default, etc.).
//!
//! The element types implement tachys' [`Render`](crate::view::Render)
//! trait, so they compose with the rest of the view-tree machinery
//! (signals, control flow, `mount_to_*`, etc.).

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

pub mod attr;
pub mod bind;
pub(crate) mod directives;
pub mod element;
pub mod node_ref;
pub mod window;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal};
pub use element::{
    button, hstack, label, stack_view, text_field, view, vstack,
};
pub use node_ref::NodeRef;
pub use window::{window, Window};

// Convenient passthrough of the most common style enums so users
// don't have to import them from `cocoa_dom::layout` separately.
pub use cocoa_dom::layout::{FlexDirection, JustifyContent};
