//! Attribute-key markers used by the `bind:` machinery in
//! [`crate::cocoa::bind`]. The platform-agnostic markers (`Value`,
//! `Checked`, `AttributeKey`, `MouseHover`) live in
//! `renderer::attr_keys`; this module re-exports them.
//!
//! There used to be a port-local `Selection` key for `bind:selection=`
//! on PopUpButton and SegmentedControl; those now route through
//! `bind:value=` (disambiguated by the signal's `usize` type).

pub use renderer::attr_keys::{AttributeKey, Checked, MouseHover, Value};
