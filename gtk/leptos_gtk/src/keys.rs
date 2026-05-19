//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::gtk::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`) live in `renderer::attr_keys`; this
//! module re-exports them.

pub use renderer::attr_keys::{AttributeKey, Checked, Value};
