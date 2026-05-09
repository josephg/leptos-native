//! Macro-facing facade for `tachys::svg` on Linux.
//!
//! The `view!{}` macro routes the `<view>` tag through
//! `tachys::svg::view` (because `view` is a real SVG tag). On GTK
//! that resolves to the gtk container builder.

#![allow(missing_docs)]

pub use crate::gtk::element::view;
