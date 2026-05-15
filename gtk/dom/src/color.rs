//! `Color` — minimal sRGB rgba shim for cross-port portability.
//!
//! GTK widgets don't accept inline colour attrs the way Cocoa /
//! iOS do — styling is meant to go through GTK CSS via
//! `gtk::CssProvider`. The `Color` type here exists so portable
//! view code can write `<vstack background_color=Color::RED>` and
//! have it *compile* on GTK; the value just goes through a
//! warn-and-discard path at install time.
//!
//! When inline-styling support lands on GTK (translating these
//! attrs to per-widget CSS), this Color type stays user-facing
//! and grows real semantics.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED: Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}
