//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::cocoa::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`) live in `leptos_apple_shared`; this
//! module re-exports them.
//!
//! There used to be a port-local `Selection` key for `bind:selection=`
//! on PopUpButton and SegmentedControl; those now route through
//! `bind:value=` (disambiguated by the signal's `usize` type).

pub use leptos_apple_shared::attr_keys::{AttributeKey, Checked, Value};
