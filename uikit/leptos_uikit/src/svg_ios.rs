//! Macro-facing facade for `tachys::svg` on iOS.
//!
//! The `view!{}` macro routes a handful of tags through `tachys::svg`
//! because they're SVG element names in the web spec — `<view>` and
//! `<switch>` among them. On iOS we repurpose those tag names:
//! `<view>` is our generic `UIView` container, `<switch>` is
//! `UISwitch`. The re-exports here let the macro's emitted
//! `tachys::svg::<tag>()` call resolve to our UIKit builders.

#![allow(missing_docs)]

pub use crate::ios::element::view;

/// `<switch>` resolves here. The function name is `switch` (a Rust
/// keyword) — declared via raw-identifier syntax, mirroring
/// `tachys::svg::r#use` in the web port.
pub fn r#switch() -> crate::ios::element::Switch {
    crate::ios::element::switch_()
}
