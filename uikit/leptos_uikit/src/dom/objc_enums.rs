//! Newtype wrappers around objc2-ui-kit enums.
//!
//! User code (and the leptos_uikit builders downstream) reaches for
//! these instead of the raw `objc2_ui_kit::*` types so the public
//! API doesn't leak the objc2 dependency. They also satisfy the
//! orphan rule for `renderer::IntoMaybeReactive` impls — the bare
//! UIKit types are foreign to every workspace crate, so blanket
//! impls would be impossible without the wrap.
//!
//! Each wrapper is `#[repr(transparent)]` over the objc2 type, so
//! conversions are zero-cost and match the underlying type's `Copy`
//! semantics.

//! Note: `IntoMaybeReactive` impls for these wrappers live in the
//! per-port `ios::attr` shadow trait, not against `renderer`'s
//! trait. See `cocoa/dom/src/objc_enums.rs` for the orphan-rule
//! explanation.

use objc2_ui_kit::{NSTextAlignment, UIBlurEffectStyle, UIDatePickerStyle};

/// Text alignment within a label / text field / text view. iOS shares
/// the `NSTextAlignment` enum with macOS via Foundation; the variants
/// are the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct TextAlignment(pub NSTextAlignment);

impl TextAlignment {
    pub const LEFT: Self = Self(NSTextAlignment::Left);
    pub const RIGHT: Self = Self(NSTextAlignment::Right);
    pub const CENTER: Self = Self(NSTextAlignment::Center);
    pub const JUSTIFIED: Self = Self(NSTextAlignment::Justified);
    pub const NATURAL: Self = Self(NSTextAlignment::Natural);
}

impl From<NSTextAlignment> for TextAlignment {
    fn from(v: NSTextAlignment) -> Self { Self(v) }
}

/// `UIDatePicker` visual style — automatic / wheels / compact /
/// inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct DatePickerStyle(pub UIDatePickerStyle);

impl DatePickerStyle {
    pub const AUTOMATIC: Self = Self(UIDatePickerStyle::Automatic);
    pub const WHEELS: Self = Self(UIDatePickerStyle::Wheels);
    pub const COMPACT: Self = Self(UIDatePickerStyle::Compact);
    pub const INLINE: Self = Self(UIDatePickerStyle::Inline);
}

impl From<UIDatePickerStyle> for DatePickerStyle {
    fn from(v: UIDatePickerStyle) -> Self { Self(v) }
}

/// `UIBlurEffect` style for `<blur_view>` — the adaptive "material"
/// styles plus the classic light/dark blurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct BlurStyle(pub UIBlurEffectStyle);

impl BlurStyle {
    pub const SYSTEM_ULTRA_THIN_MATERIAL: Self =
        Self(UIBlurEffectStyle::SystemUltraThinMaterial);
    pub const SYSTEM_THIN_MATERIAL: Self =
        Self(UIBlurEffectStyle::SystemThinMaterial);
    pub const SYSTEM_MATERIAL: Self = Self(UIBlurEffectStyle::SystemMaterial);
    pub const SYSTEM_THICK_MATERIAL: Self =
        Self(UIBlurEffectStyle::SystemThickMaterial);
    pub const SYSTEM_CHROME_MATERIAL: Self =
        Self(UIBlurEffectStyle::SystemChromeMaterial);
    pub const EXTRA_LIGHT: Self = Self(UIBlurEffectStyle::ExtraLight);
    pub const LIGHT: Self = Self(UIBlurEffectStyle::Light);
    pub const DARK: Self = Self(UIBlurEffectStyle::Dark);
    pub const REGULAR: Self = Self(UIBlurEffectStyle::Regular);
    pub const PROMINENT: Self = Self(UIBlurEffectStyle::Prominent);
}

impl From<UIBlurEffectStyle> for BlurStyle {
    fn from(v: UIBlurEffectStyle) -> Self { Self(v) }
}

