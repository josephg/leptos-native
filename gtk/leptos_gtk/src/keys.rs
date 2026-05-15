//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::gtk::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`) live in `leptos_apple_shared` (which
//! despite the name is platform-neutral); this module re-exports
//! them.

pub use leptos_apple_shared::attr_keys::{AttributeKey, Checked, Value};
