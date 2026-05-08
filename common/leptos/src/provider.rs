//! Phase 7 stub. The original `Provider` component used `TypedChildren`
//! (from the deferred `children` module) and `OwnedView` (from
//! `tachys::reactive_graph::OwnedView`, which was deleted in Phase 5
//! along with the heavily-RenderHtml-coupled `tachys` modules).
//!
//! Phase 8 will:
//! - Re-add a renderer-agnostic `OwnedView` wrapper to `common/renderer`
//!   (it sets the reactive `Owner` per `build`/`rebuild`, no web specifics).
//! - Re-add `TypedChildren` to a Phase-8 children module against
//!   `IntoView<R>`.
//! - Restore `Provider` here against the new shape.

// Intentionally empty pending Phase 8.
