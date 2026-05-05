//! Macro-facing facade for `tachys::html::element` on iOS.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects, backed by the UIKit builders in `tachys::ios::element`.

#![allow(missing_docs)]

pub use crate::ios::element::{
    button, hstack, label, secure_text_field, slider, switch_,
    text_field, vstack,
};

// `<div>` aliases the generic UIView container.
pub use crate::ios::element::view as div;

// Note: `<view>` itself is special-cased in the macro as an SVG
// element, routed through `tachys::svg::view`. The iOS facade for
// `tachys::svg` re-routes to our UIView builder.
