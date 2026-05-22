//! `use:directive=param` macro plumbing for iOS builders. The
//! generic `pack` and `run_all` helpers live in
//! `renderer::directive`; this module just re-exports them at the
//! path the iOS builders import from. Currently unreferenced — the
//! iOS builders have no `.directive()` method yet (see
//! `audit_ios.md` issue 5d). Kept so wiring it up later is a
//! one-side change.

#![allow(unused_imports)]

pub(crate) use leptos_native::renderer::directive::{pack, run_all};
