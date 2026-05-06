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
    button, checkbox, color_well, date_picker, hstack, image_view, label,
    pop_up_button, progress_indicator, scroll_view, secure_text_field,
    segmented_control, slider, stack, stack_view, stepper, text_field,
    text_view, vstack,
};

#[cfg(feature = "block_layout")]
pub use crate::cocoa::element::block;

// Note: `<view>` itself is special-cased in the macro as an SVG
// element (`view` is a real SVG tag), so the macro emits
// `tachys::svg::view` rather than `tachys::html::element::view`. We
// handle that in `tachys::svg` (the macOS facade for `tachys::svg`)
// — `<view>` resolves there.
