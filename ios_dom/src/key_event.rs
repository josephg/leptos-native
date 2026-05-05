//! Web-shaped keyboard event payload.
//!
//! Full `UIKeyCommand` / `pressesBegan:` integration is deferred to
//! Stage 8 — until then this struct only exists to match the cocoa
//! port's API surface. `Element::on_text_keydown` /
//! `on_text_keyup` are no-op stubs.

#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key: String,
    pub key_code: u32,
}
