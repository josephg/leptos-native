//! `Color` — the iOS port's color type.
//!
//! Variants:
//!   * [`Color::Rgba`] — a fixed sRGB value, the same in light and
//!     dark mode.
//!   * [`Color::System`] — one of UIKit's named adaptive colors
//!     (`label`, `systemBackground`, `systemBlue`, etc.).
//!     Resolves to a different concrete colour in dark mode
//!     automatically; UIKit re-resolves on
//!     `traitCollectionDidChange:` without our code having to
//!     re-fire the corresponding effects, because
//!     `[UILabel setTextColor:]` (and friends) store the dynamic
//!     `UIColor` reference and re-read it on every redraw.
//!
//! [Apple's adaptive-color guide][1] is the canonical taxonomy.
//!
//! [1]: https://developer.apple.com/design/human-interface-guidelines/foundations/color

use objc2::rc::Retained;
use objc2_ui_kit::UIColor;

/// A colour value that resolves to a `UIColor` on demand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    /// Fixed sRGB. Components are 0.0..=1.0.
    Rgba { r: f32, g: f32, b: f32, a: f32 },
    /// One of UIKit's named adaptive colors (light- and dark-mode
    /// aware).
    System(SystemColor),
    /// A custom light/dark pair, resolved per trait collection via
    /// `UIColor(dynamicProvider:)` — re-resolves automatically on
    /// appearance changes, like [`Color::System`]. Components are
    /// `[r, g, b, a]` in 0.0..=1.0.
    Dynamic { light: [f32; 4], dark: [f32; 4] },
}

/// Named UIKit adaptive colors. See
/// <https://developer.apple.com/design/human-interface-guidelines/foundations/color>.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum SystemColor {
    // --- Text ---
    Label,
    SecondaryLabel,
    TertiaryLabel,
    QuaternaryLabel,
    PlaceholderText,
    Link,
    // --- Separators ---
    Separator,
    OpaqueSeparator,
    // --- Backgrounds ---
    SystemBackground,
    SecondarySystemBackground,
    TertiarySystemBackground,
    SystemGroupedBackground,
    SecondarySystemGroupedBackground,
    TertiarySystemGroupedBackground,
    // --- Fills (overlays / tints) ---
    SystemFill,
    SecondarySystemFill,
    TertiarySystemFill,
    QuaternarySystemFill,
    // --- Brand-coloured ---
    SystemRed,
    SystemOrange,
    SystemYellow,
    SystemGreen,
    SystemTeal,
    SystemBlue,
    SystemIndigo,
    SystemPurple,
    SystemPink,
    SystemBrown,
    // --- Greys ---
    SystemGray,
    SystemGray2,
    SystemGray3,
    SystemGray4,
    SystemGray5,
    SystemGray6,
    /// The app's tint color (configurable per-window).
    Tint,
}

impl Color {
    pub const WHITE: Self = Self::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED: Self = Self::Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Self = Self::Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Self = Self::Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

    // Convenience constants for the most common adaptive colors.
    pub const LABEL: Self = Self::System(SystemColor::Label);
    pub const SECONDARY_LABEL: Self = Self::System(SystemColor::SecondaryLabel);
    pub const SYSTEM_BACKGROUND: Self =
        Self::System(SystemColor::SystemBackground);
    pub const SYSTEM_BLUE: Self = Self::System(SystemColor::SystemBlue);
    pub const SYSTEM_YELLOW: Self = Self::System(SystemColor::SystemYellow);
    pub const SYSTEM_RED: Self = Self::System(SystemColor::SystemRed);
    pub const SYSTEM_GREEN: Self = Self::System(SystemColor::SystemGreen);

    /// Fixed sRGB rgb (alpha = 1).
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::Rgba { r, g, b, a: 1.0 }
    }

    /// Fixed sRGB rgba.
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::Rgba { r, g, b, a }
    }

    /// A light/dark adaptive pair from two fixed colors. Falls back
    /// to `light` if either argument is not a plain [`Color::Rgba`].
    pub fn dynamic(light: Self, dark: Self) -> Self {
        match (light, dark) {
            (
                Self::Rgba { r, g, b, a },
                Self::Rgba { r: dr, g: dg, b: db, a: da },
            ) => Self::Dynamic {
                light: [r, g, b, a],
                dark: [dr, dg, db, da],
            },
            _ => light,
        }
    }

    /// Render this colour as a `UIColor`. For `System` variants
    /// the returned `UIColor` is dynamic — UIKit picks the correct
    /// concrete value at draw time based on the surrounding view's
    /// `traitCollection.userInterfaceStyle`.
    pub fn to_uicolor(self) -> Retained<UIColor> {
        match self {
            Self::Rgba { r, g, b, a } => {
                UIColor::colorWithRed_green_blue_alpha(
                    r as f64, g as f64, b as f64, a as f64,
                )
            }
            Self::Dynamic { light, dark } => {
                use core::ptr::NonNull;
                use objc2_ui_kit::{UITraitCollection, UIUserInterfaceStyle};

                let mk = |c: [f32; 4]| {
                    UIColor::colorWithRed_green_blue_alpha(
                        c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64,
                    )
                };
                // Both concrete colors are owned by the provider
                // block; the dynamic UIColor retains the block, so
                // the returned borrows stay valid for its lifetime.
                let light = mk(light);
                let dark = mk(dark);
                let block = block2::RcBlock::new(
                    move |traits: NonNull<UITraitCollection>| {
                        let style = unsafe {
                            traits.as_ref().userInterfaceStyle()
                        };
                        let chosen: &UIColor =
                            if style == UIUserInterfaceStyle::Dark {
                                &dark
                            } else {
                                &light
                            };
                        NonNull::from(chosen)
                    },
                );
                unsafe { UIColor::colorWithDynamicProvider(&block) }
            }
            Self::System(s) => match s {
                SystemColor::Label => UIColor::labelColor(),
                SystemColor::SecondaryLabel => UIColor::secondaryLabelColor(),
                SystemColor::TertiaryLabel => UIColor::tertiaryLabelColor(),
                SystemColor::QuaternaryLabel => UIColor::quaternaryLabelColor(),
                SystemColor::PlaceholderText => UIColor::placeholderTextColor(),
                SystemColor::Link => UIColor::linkColor(),
                SystemColor::Separator => UIColor::separatorColor(),
                SystemColor::OpaqueSeparator => UIColor::opaqueSeparatorColor(),
                SystemColor::SystemBackground => UIColor::systemBackgroundColor(),
                SystemColor::SecondarySystemBackground => UIColor::secondarySystemBackgroundColor(),
                SystemColor::TertiarySystemBackground => UIColor::tertiarySystemBackgroundColor(),
                SystemColor::SystemGroupedBackground => UIColor::systemGroupedBackgroundColor(),
                SystemColor::SecondarySystemGroupedBackground => UIColor::secondarySystemGroupedBackgroundColor(),
                SystemColor::TertiarySystemGroupedBackground => UIColor::tertiarySystemGroupedBackgroundColor(),
                SystemColor::SystemFill => UIColor::systemFillColor(),
                SystemColor::SecondarySystemFill => UIColor::secondarySystemFillColor(),
                SystemColor::TertiarySystemFill => UIColor::tertiarySystemFillColor(),
                SystemColor::QuaternarySystemFill => UIColor::quaternarySystemFillColor(),
                SystemColor::SystemRed => UIColor::systemRedColor(),
                SystemColor::SystemOrange => UIColor::systemOrangeColor(),
                SystemColor::SystemYellow => UIColor::systemYellowColor(),
                SystemColor::SystemGreen => UIColor::systemGreenColor(),
                SystemColor::SystemTeal => UIColor::systemTealColor(),
                SystemColor::SystemBlue => UIColor::systemBlueColor(),
                SystemColor::SystemIndigo => UIColor::systemIndigoColor(),
                SystemColor::SystemPurple => UIColor::systemPurpleColor(),
                SystemColor::SystemPink => UIColor::systemPinkColor(),
                SystemColor::SystemBrown => UIColor::systemBrownColor(),
                SystemColor::SystemGray => UIColor::systemGrayColor(),
                SystemColor::SystemGray2 => UIColor::systemGray2Color(),
                SystemColor::SystemGray3 => UIColor::systemGray3Color(),
                SystemColor::SystemGray4 => UIColor::systemGray4Color(),
                SystemColor::SystemGray5 => UIColor::systemGray5Color(),
                SystemColor::SystemGray6 => UIColor::systemGray6Color(),
                SystemColor::Tint => UIColor::tintColor(),
            },
        }
    }

    /// Read components off a `UIColor` via `getRed:green:blue:alpha:`.
    /// Only meaningful for non-dynamic colors — system / dynamic
    /// colors return `None` because their resolved value depends on
    /// the trait collection of the surrounding view.
    pub fn from_uicolor(c: &UIColor) -> Option<Self> {
        let mut r: f64 = 0.0;
        let mut g: f64 = 0.0;
        let mut b: f64 = 0.0;
        let mut a: f64 = 0.0;
        let success = unsafe {
            c.getRed_green_blue_alpha(&mut r, &mut g, &mut b, &mut a)
        };
        if success {
            Some(Self::Rgba {
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
