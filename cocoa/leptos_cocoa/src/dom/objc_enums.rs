//! Newtype wrappers around objc2-app-kit enums.
//!
//! User code (and the leptos_cocoa builders downstream) reaches for
//! these instead of the raw `objc2_app_kit::NS*` types so the public
//! API doesn't leak the objc2 dependency. They also satisfy the
//! orphan rule for `renderer::IntoMaybeReactive` impls — the bare
//! `NS*` types are foreign to every workspace crate, so blanket
//! impls would be impossible without the wrap.
//!
//! Each wrapper is `#[repr(transparent)]` over the objc2 enum, so
//! conversions are zero-cost and match the underlying enum's `Copy`
//! semantics.

//! Note: `IntoMaybeReactive` impls for these wrappers live in the
//! per-port `cocoa::attr` shadow trait, not against `renderer`'s
//! trait. Rust's orphan rule blocks closure-form impls (`impl<F:
//! Fn() -> Local> renderer::IntoMaybeReactive<Local> for F`) from
//! crates that don't own the trait — `F` is the impl's Self type
//! and appears before the trait's first local type parameter. The
//! shadow trait dodges this by being itself local.

use objc2_app_kit::{NSDatePickerStyle, NSLineBreakMode, NSSegmentStyle, NSTextAlignment};

/// Text alignment within a label / text-field / text-view.
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

/// `NSSegmentedControl` visual style — Rounded, RoundRect, Capsule,
/// SmallSquare, etc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SegmentStyle(pub NSSegmentStyle);

impl SegmentStyle {
    pub const AUTOMATIC: Self = Self(NSSegmentStyle::Automatic);
    pub const ROUNDED: Self = Self(NSSegmentStyle::Rounded);
    pub const ROUND_RECT: Self = Self(NSSegmentStyle::RoundRect);
    pub const TEXTURED_SQUARE: Self = Self(NSSegmentStyle::TexturedSquare);
    pub const SMALL_SQUARE: Self = Self(NSSegmentStyle::SmallSquare);
    pub const CAPSULE: Self = Self(NSSegmentStyle::Capsule);
    pub const TEXTURED_ROUNDED: Self = Self(NSSegmentStyle::TexturedRounded);
    pub const SEPARATED: Self = Self(NSSegmentStyle::Separated);
}

impl From<NSSegmentStyle> for SegmentStyle {
    fn from(v: NSSegmentStyle) -> Self { Self(v) }
}

/// `NSDatePicker` visual style — textual / textual+stepper /
/// clock-and-calendar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct DatePickerStyle(pub NSDatePickerStyle);

impl DatePickerStyle {
    pub const TEXT_FIELD_AND_STEPPER: Self =
        Self(NSDatePickerStyle::TextFieldAndStepper);
    pub const CLOCK_AND_CALENDAR: Self =
        Self(NSDatePickerStyle::ClockAndCalendar);
    pub const TEXT_FIELD: Self = Self(NSDatePickerStyle::TextField);
}

impl From<NSDatePickerStyle> for DatePickerStyle {
    fn from(v: NSDatePickerStyle) -> Self { Self(v) }
}

/// How a label / text-field handles text that doesn't fit. Maps
/// directly to `NSLineBreakMode`.
///
/// - `CLIP` — silently truncate, no indication.
/// - `TRUNCATE_HEAD` / `TRUNCATE_TAIL` / `TRUNCATE_MIDDLE` — drop
///   content from that end and show an ellipsis. `TRUNCATE_TAIL`
///   is the most common for app titles / list rows.
/// - `WORD_WRAP` / `CHAR_WRAP` — wrap across multiple lines.
///   `WORD_WRAP` is what `Label::multiline(true)` selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct LineBreak(pub NSLineBreakMode);

impl LineBreak {
    pub const WORD_WRAP:       Self = Self(NSLineBreakMode::ByWordWrapping);
    pub const CHAR_WRAP:       Self = Self(NSLineBreakMode::ByCharWrapping);
    pub const CLIP:            Self = Self(NSLineBreakMode::ByClipping);
    pub const TRUNCATE_HEAD:   Self = Self(NSLineBreakMode::ByTruncatingHead);
    pub const TRUNCATE_TAIL:   Self = Self(NSLineBreakMode::ByTruncatingTail);
    pub const TRUNCATE_MIDDLE: Self = Self(NSLineBreakMode::ByTruncatingMiddle);
}

impl From<NSLineBreakMode> for LineBreak {
    fn from(v: NSLineBreakMode) -> Self { Self(v) }
}

