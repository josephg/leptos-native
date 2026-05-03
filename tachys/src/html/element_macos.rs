//! Macro-facing facade for `tachys::html::element` on macOS.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects (`::leptos::tachys::html::element::<tag>()`), backed by
//! the Cocoa builders in `tachys::cocoa::element`.
//!
//! This is "Option 2" from the Stage-5-part-3 design: the macro
//! emission stays unchanged; the path it emits resolves on macOS to
//! a Cocoa-flavoured implementation that quacks the way the macro
//! expects (chained `.child(...)` / `.on(event, handler)`).

#![allow(missing_docs)]

// Direct re-exports for tags that map 1:1 to a Cocoa builder.
pub use crate::cocoa::element::{
    button, checkbox, hstack, label, pop_up_button, secure_text_field, slider,
    stack_view, text_field, vstack,
};

// `<div>` aliases the generic flipped container. Common HTML idiom;
// keeping it lets users coming from web code reach for what they
// know.
pub use crate::cocoa::element::view as div;

// Note: `<view>` itself is special-cased in the macro as an SVG
// element (`view` is a real SVG tag), so the macro emits
// `tachys::svg::view` rather than `tachys::html::element::view`. We
// handle that in `tachys::svg` (the macOS facade for `tachys::svg`)
// — `<view>` resolves there.
