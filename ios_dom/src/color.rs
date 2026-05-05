//! `Color` — an sRGB rgba 4-tuple, the iOS port's stand-in for
//! `web_sys::CssRgba` etc.
//!
//! Components are 0.0..=1.0 floats. Convert to/from `UIColor`.

use objc2_ui_kit::UIColor;

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

    pub fn to_uicolor(self) -> objc2::rc::Retained<UIColor> {
        UIColor::colorWithRed_green_blue_alpha(
            self.r as f64,
            self.g as f64,
            self.b as f64,
            self.a as f64,
        )
    }

    /// Read components off a `UIColor`. Uses `getRed:green:blue:alpha:`.
    pub fn from_uicolor(c: &UIColor) -> Option<Self> {
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;
        let success = unsafe { c.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a) };
        if success {
            Some(Self {
                r: r as f32,
                g: g as f32,
                b: b as f32,
                a: a as f32,
            })
        } else {
            None
        }
    }
}
