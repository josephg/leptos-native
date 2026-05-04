//! `Color` — an sRGB rgba 4-tuple, the macOS port's stand-in for
//! `web_sys::CssRgba` etc. Used as the value type for `bind:color` on
//! `<color_well>`.
//!
//! Components are 0.0..=1.0 floats. Helpers convert to/from
//! `NSColor` by way of the sRGB colorspace.

use objc2_app_kit::NSColor;

/// 4-tuple sRGB color. All components clamped to 0.0..=1.0 by
/// AppKit on the way through `NSColor`; we don't enforce here.
#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Build an `NSColor` in the sRGB colorspace from this `Color`.
    pub fn to_nscolor(self) -> objc2::rc::Retained<NSColor> {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            self.r as f64,
            self.g as f64,
            self.b as f64,
            self.a as f64,
        )
    }

    /// Read components off an `NSColor`. The picker may produce
    /// colors in arbitrary colorspaces (Display P3, etc.); we
    /// convert to sRGB first via `usingColorSpace:`. If conversion
    /// fails — extremely rare in practice — return None.
    pub fn from_nscolor(c: &NSColor) -> Option<Self> {
        let srgb_space = objc2_app_kit::NSColorSpace::sRGBColorSpace();
        let in_srgb = c.colorUsingColorSpace(&srgb_space)?;
        Some(Self {
            r: in_srgb.redComponent() as f32,
            g: in_srgb.greenComponent() as f32,
            b: in_srgb.blueComponent() as f32,
            a: in_srgb.alphaComponent() as f32,
        })
    }
}
