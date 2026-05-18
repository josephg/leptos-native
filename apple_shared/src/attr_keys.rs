//! Attribute-key marker types used by the `bind:` machinery in each
//! port. Vendored from the deleted `tachys::html::attribute::*`.
//!
//! Native targets use these markers solely to disambiguate
//! `BindAttribute<Key, Sig>` impls per-control (so a control can say
//! it supports `bind:value` but not `bind:checked`, etc.). They are
//! never used to read or write an actual element attribute.

/// Marker trait identifying an HTML-style attribute by its name.
pub trait AttributeKey: Clone + Send + 'static {
    /// The name of the attribute (informational; not used to set
    /// anything on a real element).
    const KEY: &'static str;
}

/// `bind:value` key — text fields, sliders, steppers, etc.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Value;

impl AttributeKey for Value {
    const KEY: &'static str = "value";
}

/// `bind:checked` key — checkboxes / switches.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Checked;

impl AttributeKey for Checked {
    const KEY: &'static str = "checked";
}

/// `bind:mouse_hover` key — one-way (framework → app) hover
/// state. The signal is `set(true)` when the cursor enters the
/// element and `set(false)` when it leaves. Distinct from a
/// generic `bind:value` because it ignores writes from the app
/// (cursor position is OS-driven, not app-driven).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MouseHover;

impl AttributeKey for MouseHover {
    const KEY: &'static str = "mouse_hover";
}

// Note: each port defines its own `Selection` marker in its
// `bind` module — the type is a tag for the BindAttribute trait
// dispatch and naturally lives next to the impls.
