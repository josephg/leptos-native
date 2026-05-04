//! Web-shaped keyboard event payload, plus a translator from
//! AppKit's "command selectors" into [`KeyEvent`].
//!
//! AppKit's text-input pipeline routes special keys (Return,
//! Escape, Tab, arrow keys, etc.) through
//! `NSControlTextEditingDelegate::control:textView:doCommandBySelector:`.
//! Each such key fires a *named selector* — `insertNewline:` for
//! Return, `cancelOperation:` for Escape, etc. We translate those
//! selectors into a [`KeyEvent`] whose shape matches
//! `web_sys::KeyboardEvent` closely enough that example code which
//! checks `ev.key()` or `ev.key_code()` ports across unchanged.
//!
//! ## Coverage
//!
//! Only "command keys" are reported — printable-character
//! keystrokes don't go through `doCommandBySelector:` and are not
//! captured here. (For TodoMVC and similar use cases the command
//! keys cover the entire need.) If a future example needs every
//! keystroke we'll need an NSResponder subclass overriding
//! `keyDown:` / `keyUp:` directly.
//!
//! ## keydown vs keyup
//!
//! AppKit's field-editor command pipeline doesn't distinguish
//! down from up — there's a single notification per command. We
//! fire BOTH `on:keydown` and `on:keyup` callbacks on the same
//! notification, so example code targeting either one works. A
//! field handler installed for both events on the same key would
//! see two fires per keystroke — uncommon and documented.

use objc2::{runtime::Sel, sel};

/// Keyboard-event payload passed to `on:keydown` / `on:keyup`
/// callbacks. Web parity: matches `web_sys::KeyboardEvent`'s
/// `.key()` (string name) and `.key_code()` (numeric code) fields.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Web-style key name. "Enter", "Escape", "Tab", "ArrowUp",
    /// "ArrowDown", "ArrowLeft", "ArrowRight", "Backspace",
    /// "Delete", or "" for an unrecognized selector.
    pub key: String,
    /// Web-style numeric key code (e.g. 13 for Enter, 27 for
    /// Escape). 0 for unrecognized.
    pub key_code: u32,
}

impl KeyEvent {
    /// Translate a "command selector" (the kind passed to
    /// `control:textView:doCommandBySelector:`) into a [`KeyEvent`].
    /// Returns `None` if the selector isn't one we recognize, so
    /// callers can skip firing.
    pub(crate) fn from_command_selector(s: Sel) -> Option<Self> {
        let (key, key_code): (&str, u32) = if s == sel!(insertNewline:)
            || s == sel!(insertLineBreak:)
            || s == sel!(insertNewlineIgnoringFieldEditor:)
        {
            ("Enter", 13)
        } else if s == sel!(cancelOperation:) || s == sel!(complete:) {
            ("Escape", 27)
        } else if s == sel!(insertTab:) || s == sel!(insertBacktab:) {
            ("Tab", 9)
        } else if s == sel!(moveUp:) {
            ("ArrowUp", 38)
        } else if s == sel!(moveDown:) {
            ("ArrowDown", 40)
        } else if s == sel!(moveLeft:) {
            ("ArrowLeft", 37)
        } else if s == sel!(moveRight:) {
            ("ArrowRight", 39)
        } else if s == sel!(deleteBackward:) {
            ("Backspace", 8)
        } else if s == sel!(deleteForward:) {
            ("Delete", 46)
        } else {
            return None;
        };
        Some(KeyEvent {
            key: key.to_string(),
            key_code,
        })
    }
}
