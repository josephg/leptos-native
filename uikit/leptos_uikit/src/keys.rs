//! Attribute-key marker types used by the `bind:value` /
//! `bind:checked` / `bind:selection` machinery in
//! [`crate::ios::bind`]. Vendored locally from the deleted
//! `tachys::html::attribute::*` (Phase 8 fork).

pub trait AttributeKey: Clone + Send + 'static {
    const KEY: &'static str;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Value;

impl AttributeKey for Value {
    const KEY: &'static str = "value";
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Checked;

impl AttributeKey for Checked {
    const KEY: &'static str = "checked";
}

/// `bind:selection` key — re-exported from `crate::ios::bind` so the
/// macro path `::leptos::attr::Selection` resolves.
pub use crate::ios::bind::Selection;
