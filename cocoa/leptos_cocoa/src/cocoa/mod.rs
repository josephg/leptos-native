//! Cocoa-flavoured element builders, the macOS analogue of
//! [`crate::html::element`]. Each tag here corresponds 1:1 to an
//! AppKit class; tag names drop the `NS` prefix and are snake_case
//! (`button` → NSButton, `stack` → flipped NSView with flex layout,
//! etc.).
//!
//! The element types implement tachys' [`Render`](crate::view::Render)
//! trait, so they compose with the rest of the view-tree machinery
//! (signals, control flow, `mount_to_*`, etc.).

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

#[cfg(feature = "animation")]
pub mod animation;
pub mod attr;
pub mod bind;
pub(crate) mod directives;
pub mod element;
pub(crate) mod error_guard;
pub mod menu;
pub mod node_ref;
pub mod split;
pub mod toolbar;
pub mod window;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal};
pub use element::{
    button, grid, hstack, label, stack, text_field, vstack,
};
pub use node_ref::NodeRef;
pub use window::{window, Window};

// Convenient passthrough of the most common style enums so users
// don't have to import them from `cocoa_dom::layout` separately.
pub use cocoa_dom::layout::{
    AlignContent, AlignItems, FlexDirection, FlexWrap, GridAutoFlow,
    GridTemplateComponent, JustifyContent, JustifyItems, TrackSizingFunction,
};
