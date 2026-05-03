//! Macro-facing facade for `tachys::svg` on macOS.
//!
//! The `view!{}` macro routes the `<view>` tag (which is a real SVG
//! tag in the web spec) to `tachys::svg::view()`. On macOS we
//! repurpose `<view>` to mean "the generic flipped NSView container"
//! — it's the natural Cocoa idiom — by aliasing `tachys::svg::view`
//! to our cocoa view builder here.
//!
//! Other SVG tags aren't supported on macOS (no SVG rendering
//! pipeline). If a user's `view!{}` invocation references one, they'll
//! get a "cannot find function in module" error pointing at the
//! offending tag — reasonable failure mode.

#![allow(missing_docs)]

pub use crate::cocoa::element::view;
