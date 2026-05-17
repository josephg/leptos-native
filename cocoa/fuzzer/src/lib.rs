//! Cocoa correctness fuzzer.
//!
//! Pipeline (per seed):
//! 1. Generate a random [`spec::Node`] via [`generator::Generator`].
//! 2. Build a *reactive* cocoa view from it via [`render::build`],
//!    wiring some attrs to `RwSignal`s in a [`signals::SignalStore`].
//! 3. Mount the reactive view under a fresh `OpenedWindow`'s
//!    content_root and run an initial Taffy layout pass.
//! 4. Run the [`chaos::Chaos`] loop: many random signal mutations,
//!    pumping the run-loop in between so RenderEffects deliver.
//! 5. Snapshot the final signal values, restore them onto a fresh
//!    `SignalStore`, and build a *static* tree against a second
//!    `OpenedWindow`. Lay out at the same size.
//! 6. Walk both NSView hierarchies via [`compare::compare_trees`].
//!    Any structural or attribute mismatch returns `Err`.

pub mod chaos;
pub mod compare;
pub mod generator;
pub mod interact;
pub mod render;
pub mod signals;
pub mod spec;
