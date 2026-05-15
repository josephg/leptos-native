//! `Color` — the Cocoa port's color type.
//!
//! Variants:
//!   * [`Color::Rgba`] — a fixed sRGB value, the same in light and
//!     dark mode.
//!   * [`Color::System`] — one of AppKit's named adaptive colors
//!     (`labelColor`, `controlBackgroundColor`, etc.). Resolves
//!     to a different concrete colour automatically based on the
//!     current `NSAppearance`. AppKit re-resolves dynamic
//!     `NSColor`s on redraw, so a view configured with a system
//!     colour follows light/dark mode without our code having to
//!     re-fire effects.
//!
//! Mirrors `ios_dom::Color`'s shape. [Apple's adaptive-color
//! guide][1] is the canonical taxonomy.
//!
//! [1]: https://developer.apple.com/design/human-interface-guidelines/foundations/color

use objc2_app_kit::NSColor;

/// A colour value that resolves to an `NSColor` on demand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    /// Fixed sRGB. Components are 0.0..=1.0.
    Rgba { r: f32, g: f32, b: f32, a: f32 },
    /// One of AppKit's named adaptive colors (light- and dark-mode
    /// aware).
    System(SystemColor),
}

/// Named AppKit adaptive colors. The naming follows iOS's
/// `SystemColor` where AppKit has a direct equivalent; AppKit-only
/// names (`ControlBackground`, `WindowBackground`, etc.) match
/// their `NSColor` class-method names.
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
    // --- Backgrounds ---
    /// Equivalent to UIKit's `systemBackground`.
    WindowBackground,
    /// AppKit-style "control" backdrop (e.g. behind buttons).
    ControlBackground,
    /// Background of a text-input field.
    TextBackground,
    /// Light-coloured "alternating row" backdrop, lighter shade.
    UnderPageBackground,
    /// Selection highlight.
    SelectedContentBackground,
    // --- Fills / control content ---
    ControlAccent,
    SelectedControl,
    // --- Brand-coloured (match UIKit names) ---
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
    SystemGray,
}

impl Color {
    // --- Fixed sRGB constants ---
    pub const WHITE: Self = Self::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED: Self = Self::Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Self = Self::Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Self = Self::Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const YELLOW: Self = Self::Rgba { r: 1.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const CYAN: Self = Self::Rgba { r: 0.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const MAGENTA: Self = Self::Rgba { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const ORANGE: Self = Self::Rgba { r: 1.0, g: 0.5, b: 0.0, a: 1.0 };
    pub const PURPLE: Self = Self::Rgba { r: 0.5, g: 0.0, b: 0.5, a: 1.0 };
    pub const GRAY: Self = Self::Rgba { r: 0.5, g: 0.5, b: 0.5, a: 1.0 };
    pub const TRANSPARENT: Self = Self::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    // --- Adaptive-color shortcuts (parity with iOS) ---
    pub const LABEL: Self = Self::System(SystemColor::Label);
    pub const SECONDARY_LABEL: Self = Self::System(SystemColor::SecondaryLabel);
    pub const TERTIARY_LABEL: Self = Self::System(SystemColor::TertiaryLabel);
    pub const PLACEHOLDER_TEXT: Self = Self::System(SystemColor::PlaceholderText);
    pub const LINK: Self = Self::System(SystemColor::Link);
    pub const SEPARATOR: Self = Self::System(SystemColor::Separator);
    pub const WINDOW_BACKGROUND: Self =
        Self::System(SystemColor::WindowBackground);
    /// Alias for `WINDOW_BACKGROUND`, matching UIKit's name.
    pub const SYSTEM_BACKGROUND: Self =
        Self::System(SystemColor::WindowBackground);
    pub const CONTROL_BACKGROUND: Self =
        Self::System(SystemColor::ControlBackground);
    pub const TEXT_BACKGROUND: Self = Self::System(SystemColor::TextBackground);
    pub const CONTROL_ACCENT: Self = Self::System(SystemColor::ControlAccent);
    pub const SYSTEM_RED: Self = Self::System(SystemColor::SystemRed);
    pub const SYSTEM_ORANGE: Self = Self::System(SystemColor::SystemOrange);
    pub const SYSTEM_YELLOW: Self = Self::System(SystemColor::SystemYellow);
    pub const SYSTEM_GREEN: Self = Self::System(SystemColor::SystemGreen);
    pub const SYSTEM_TEAL: Self = Self::System(SystemColor::SystemTeal);
    pub const SYSTEM_BLUE: Self = Self::System(SystemColor::SystemBlue);
    pub const SYSTEM_INDIGO: Self = Self::System(SystemColor::SystemIndigo);
    pub const SYSTEM_PURPLE: Self = Self::System(SystemColor::SystemPurple);
    pub const SYSTEM_PINK: Self = Self::System(SystemColor::SystemPink);
    pub const SYSTEM_BROWN: Self = Self::System(SystemColor::SystemBrown);
    pub const SYSTEM_GRAY: Self = Self::System(SystemColor::SystemGray);

    /// Fixed sRGB rgb (alpha = 1).
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::Rgba { r, g, b, a: 1.0 }
    }

    /// Fixed sRGB rgba.
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self::Rgba { r, g, b, a }
    }

    /// Build an `NSColor` from this `Color`. For `System` variants
    /// the returned `NSColor` is dynamic — AppKit resolves it at
    /// draw time based on the surrounding view's `NSAppearance`.
    pub fn to_nscolor(self) -> objc2::rc::Retained<NSColor> {
        match self {
            Self::Rgba { r, g, b, a } => {
                NSColor::colorWithSRGBRed_green_blue_alpha(
                    r as f64,
                    g as f64,
                    b as f64,
                    a as f64,
                )
            }
            Self::System(s) => system_to_nscolor(s),
        }
    }

    /// Read components off an `NSColor`. The picker may produce
    /// colors in arbitrary colorspaces (Display P3, etc.); we
    /// convert to sRGB first. Returns an `Rgba` variant; dynamic
    /// system colours aren't round-tripped (they'd lose their
    /// dynamic behaviour anyway).
    pub fn from_nscolor(c: &NSColor) -> Option<Self> {
        let srgb_space = objc2_app_kit::NSColorSpace::sRGBColorSpace();
        let in_srgb = c.colorUsingColorSpace(&srgb_space)?;
        Some(Self::Rgba {
            r: in_srgb.redComponent() as f32,
            g: in_srgb.greenComponent() as f32,
            b: in_srgb.blueComponent() as f32,
            a: in_srgb.alphaComponent() as f32,
        })
    }
}

fn system_to_nscolor(s: SystemColor) -> objc2::rc::Retained<NSColor> {
    match s {
            SystemColor::Label => NSColor::labelColor(),
            SystemColor::SecondaryLabel => NSColor::secondaryLabelColor(),
            SystemColor::TertiaryLabel => NSColor::tertiaryLabelColor(),
            SystemColor::QuaternaryLabel => NSColor::quaternaryLabelColor(),
            SystemColor::PlaceholderText => NSColor::placeholderTextColor(),
            SystemColor::Link => NSColor::linkColor(),
            SystemColor::Separator => NSColor::separatorColor(),
            SystemColor::WindowBackground => NSColor::windowBackgroundColor(),
            SystemColor::ControlBackground => NSColor::controlBackgroundColor(),
            SystemColor::TextBackground => NSColor::textBackgroundColor(),
            SystemColor::UnderPageBackground => {
                NSColor::underPageBackgroundColor()
            }
            SystemColor::SelectedContentBackground => {
                NSColor::selectedContentBackgroundColor()
            }
            SystemColor::ControlAccent => NSColor::controlAccentColor(),
            SystemColor::SelectedControl => NSColor::selectedControlColor(),
            SystemColor::SystemRed => NSColor::systemRedColor(),
            SystemColor::SystemOrange => NSColor::systemOrangeColor(),
            SystemColor::SystemYellow => NSColor::systemYellowColor(),
            SystemColor::SystemGreen => NSColor::systemGreenColor(),
            SystemColor::SystemTeal => NSColor::systemTealColor(),
            SystemColor::SystemBlue => NSColor::systemBlueColor(),
            SystemColor::SystemIndigo => NSColor::systemIndigoColor(),
            SystemColor::SystemPurple => NSColor::systemPurpleColor(),
            SystemColor::SystemPink => NSColor::systemPinkColor(),
            SystemColor::SystemBrown => NSColor::systemBrownColor(),
            SystemColor::SystemGray => NSColor::systemGrayColor(),
        }
}
