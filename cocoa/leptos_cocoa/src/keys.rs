//! Attribute-key marker types used by the `bind:value` / `bind:checked`
//! / `bind:selection` machinery in [`crate::cocoa::bind`].
//!
//! Phase 8: previously these came from `tachys::html::attribute::*`
//! (`Value`, `Checked`, `AttributeKey`). That whole module is gone in
//! the native fork; the only pieces bind needs are the zero-sized
//! marker structs and the trait, so we vendor them here.

/// Marker trait identifying an HTML-style attribute by its name.
///
/// Native targets use these markers solely to disambiguate the
/// `BindAttribute<Key, Sig>` trait impls per-control (so a control can
/// say it supports `bind:value` but not `bind:checked`, etc.).
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

/// `bind:checked` key — checkboxes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Checked;

impl AttributeKey for Checked {
    const KEY: &'static str = "checked";
}
