//! Macro-facing facade for `tachys::html::element` on Linux.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects (`::leptos::tachys::html::element::<tag>()`), backed by
//! the GTK builders in `crate::gtk::element`.

#![allow(missing_docs)]

pub use crate::gtk::element::{
    button, checkbox, grid, hstack, label, pop_up_button, secure_text_field,
    slider, stack, stack_view, text_field, toggle, vstack,
};

// Menus: `<menu_bar>` + `<menu>` + `<menu_item>` + `<menu_separator>`.
// `<menu_bar>` sits as a top-level sibling of `<window>` in `run()`;
// the others only make sense nested inside.
pub use crate::gtk::menu::{menu, menu_bar, menu_item, menu_separator};
