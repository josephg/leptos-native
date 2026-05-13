//! Cross-backend menu-related shared types.
//!
//! Only [`Modifiers`] lives here today — a portable bag of keyboard
//! modifier flags used by `<menu_item shortcut=…>` / per-port menu
//! builders. The translation to platform-native types
//! (`NSEventModifierFlags` for AppKit, the `<Primary><Shift>` accel
//! string for GTK) is done in each port's `menu` module.
//!
//! The menu *builder* surface is per-port (cocoa's `NSMenu` and
//! GTK's `gio::Menu` model menus very differently); the only thing
//! both ports genuinely share is the modifier-bag type, so that's
//! the entire surface of this module for now.

/// Keyboard-modifier flags for a menu item's shortcut.
///
/// `command` is the "primary" platform modifier — `⌘` on macOS,
/// `Ctrl` on Linux. The other three are explicit and map 1:1 onto
/// their AppKit / GTK counterparts.
///
/// Construct via the `CMD`, `CMD_SHIFT`, etc. constants or the
/// builder methods (`Modifiers::default().shift().option()`). Defaults
/// to no modifiers — useful for items whose `shortcut` is e.g. a
/// function key.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Modifiers {
    pub command: bool,
    pub shift:   bool,
    pub option:  bool,
    pub control: bool,
}

impl Modifiers {
    /// No modifiers — same as `Modifiers::default()`.
    pub const NONE: Self = Self { command: false, shift: false, option: false, control: false };
    /// ⌘ on macOS, Ctrl on Linux.
    pub const CMD: Self = Self { command: true, shift: false, option: false, control: false };
    /// ⇧⌘ on macOS, Ctrl+Shift on Linux.
    pub const CMD_SHIFT: Self = Self { command: true, shift: true, option: false, control: false };
    /// ⌥⌘ on macOS, Ctrl+Alt on Linux.
    pub const CMD_OPT: Self = Self { command: true, shift: false, option: true, control: false };
    /// ⇧⌥⌘ on macOS, Ctrl+Shift+Alt on Linux.
    pub const CMD_SHIFT_OPT: Self = Self { command: true, shift: true, option: true, control: false };

    pub const fn shift(mut self) -> Self { self.shift = true; self }
    pub const fn option(mut self) -> Self { self.option = true; self }
    pub const fn control(mut self) -> Self { self.control = true; self }
    pub const fn command(mut self) -> Self { self.command = true; self }
}
