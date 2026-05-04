//! Macro-facing facade for `tachys::html::element` on Linux/GTK.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects (`::leptos::tachys::html::element::<tag>()`), backed by
//! the GTK builders in `tachys::gtk::element`.

#![allow(missing_docs)]

pub use crate::gtk::element::{
    button, checkbox, hstack, label, pop_up_button, secure_text_field,
    slider, stack_view, text_field, vstack,
};

// `<div>` aliases the generic vertical box container.
pub use crate::gtk::element::view as div;
