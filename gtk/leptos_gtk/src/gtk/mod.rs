//! GTK-flavoured element builders, the Linux analogue of
//! [`crate::cocoa`] in `leptos_cocoa`. Each tag corresponds 1:1 to a
//! GTK4 widget class; tag names are snake_case (`view` → flexbox
//! container, `button` → `gtk::Button`, etc.).

#![cfg(feature = "gtk")]
#![allow(missing_docs)]

pub mod attr;
pub mod bind;
pub(crate) mod directives;
pub mod element;
pub mod menu;
pub mod node_ref;
pub mod window;

pub use attr::{IntoMaybeReactive, MaybeReactive};
pub use bind::{BindAttribute, IntoSignal};
pub use element::{
    button, checkbox, grid, hstack, label, pop_up_button, secure_text_field,
    slider, stack, stack_view, text_field, view, vstack,
};
pub use node_ref::NodeRef;
pub use window::{window, Window};

// Convenient passthrough of the most common style enums.
pub use gtk_dom::layout::{
    AlignContent, AlignItems, FlexDirection, GridAutoFlow,
    GridTemplateComponent, JustifyContent, JustifyItems, TrackSizingFunction,
};
