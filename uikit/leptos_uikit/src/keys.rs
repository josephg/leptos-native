//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::ios::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`) live in `renderer::attr_keys`; this
//! module re-exports them and adds the iOS-specific `Selection`.

pub use leptos_native::renderer::attr_keys::{AttributeKey, Checked, Value};

/// `bind:selection` key — defined in `crate::ios::bind` next to its
/// `BindAttribute` impls. Re-exported here so the macro path
/// `::leptos_native::attr::Selection` resolves.
pub use crate::ios::bind::Selection;
