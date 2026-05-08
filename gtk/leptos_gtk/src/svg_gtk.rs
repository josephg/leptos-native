//! Macro-facing facade for `tachys::svg` on Linux/GTK.
//!
//! The `view!{}` macro routes the `<view>` element tag through
//! `tachys::svg::view` (because `view` is a real SVG element). On
//! GTK native we re-route it back to our box container — same
//! pattern as `tachys::svg_macos.rs`.

// Re-export the GTK `view()` function at the path the macro expects.
pub use crate::gtk::element::view;
