//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::ios::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`) live in `leptos_apple_shared`; this
//! module re-exports them and adds the iOS-specific `Selection`.

pub use leptos_apple_shared::attr_keys::{AttributeKey, Checked, Value};

/// `bind:selection` key — defined in `crate::ios::bind` next to its
/// `BindAttribute` impls. Re-exported here so the macro path
/// `::leptos::attr::Selection` resolves.
pub use crate::ios::bind::Selection;
