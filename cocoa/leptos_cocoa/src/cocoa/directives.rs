//! `use:directive=param` macro plumbing for cocoa builders. The
//! generic `pack` and `run_all` helpers live in
//! `leptos_apple_shared::directive`; this module just re-exports them
//! at the path the cocoa builders import from.

pub(crate) use leptos_apple_shared::directive::{pack, run_all};
