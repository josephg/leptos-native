//! Macro-facing facade for `tachys::html::element` on iOS.
//!
//! Re-exposes element constructors at the path the `view!{}` macro
//! expects, backed by the UIKit builders in `tachys::ios::element`.

#![allow(missing_docs)]

pub use crate::ios::element::{
    button, date_picker, grid, hstack, image_view, label, progress_indicator,
    scroll_view, secure_text_field, segmented_control, slider, stepper,
    text_field, text_view, view, vstack,
};

// `<div>` aliases the generic UIView container.
pub use crate::ios::element::view as div;

/// `<switch>` element — `switch` is a Rust keyword, so the macro
/// emits a raw identifier (`tachys::html::element::r#switch()`).
/// Delegates to the `switch_()` builder, which can't use the bare
/// name for the same reason.
pub fn r#switch() -> crate::ios::element::Switch {
    crate::ios::element::switch_()
}
