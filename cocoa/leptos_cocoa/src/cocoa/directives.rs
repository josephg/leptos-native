//! `use:directive=param` macro plumbing for cocoa builders. The
//! generic `pack` and `run_all` helpers live in
//! `renderer::directive`; this module just re-exports them at the
//! path the cocoa builders import from.

pub(crate) use leptos_native::renderer::directive::{pack, run_all};
