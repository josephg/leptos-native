//! Allows rendering user interfaces based on a statically-typed view tree.
//!
//! This view tree is generic over rendering backends. Each platform supplies
//! a `Renderer` impl; the view types are renderer-agnostic.

#![allow(incomplete_features)]
#![cfg_attr(
    all(feature = "nightly", rustc_nightly),
    feature(unsized_const_params)
)]
#![cfg_attr(all(feature = "nightly", rustc_nightly), feature(adt_const_params))]

/// Commonly-used traits.
pub mod prelude {
    pub use crate::{
        attrs::{
            AlignSelf, Dim, IntoMaybeReactive, LayoutAttrs, MaybeReactive,
            TextAttrs, UniversalAttrs, WithLayout, WithText, WithUniversal,
        },
        renderer::Renderer,
        view::{AddAnyAttr, IntoRender, Mountable, Render},
    };
}

/// Cross-backend attribute plumbing: `MaybeReactive`, `Dim`,
/// `WithLayout` / `WithUniversal` / `WithText` traits, and the
/// `LayoutAttrs` / `UniversalAttrs` / `TextAttrs` structs each
/// builder embeds.
pub mod attrs;

/// Defines the [`Renderer`](renderer::Renderer) trait — the interface each
/// platform implements to provide concrete `Element`/`Node`/`Text`/
/// `Placeholder` types.
pub mod renderer;
/// Core logic for manipulating views (`Render`, `Mountable`, view impls for
/// tuples, primitives, strings, iterators, either, fragments, keyed lists).
pub mod view;

/// Renderer-agnostic Taffy-backed layout engine — `LayoutTree<B>`
/// generic over a [`LayoutBackend`](layout::LayoutBackend), the per-port
/// `Style` re-exports, the grid track-sizing helpers, etc. Lives here
/// (not its own crate) so [`setters`]'s `IntoMaybeReactive` impls for
/// taffy types satisfy the orphan rule.
pub mod layout;

/// Generic, port-agnostic style mutators (`set_padding`,
/// `set_grid_template_columns`, …) and the trait-driven
/// `apply_layout` / `apply_universal` install loops. Each port impls
/// [`setters::LayoutNodeOps`] / [`setters::LayoutElement`] /
/// [`setters::UniversalElement`] for its node / element types and
/// reuses the generic functions.
pub mod setters;

// Mirror the old `native_layout` crate root: re-export every layout
// + setters item at the renderer root so consumer paths
// (`use renderer::{Style, set_padding, LayoutNodeOps}`) match the
// shape the per-port code already uses.
pub use layout::*;
pub use setters::*;

pub use either_of as either;

/// View implementations for the `reactive_graph` crate (closures as reactive
/// children, signals as reactive attribute values).
#[cfg(feature = "reactive_graph")]
pub mod reactive_graph;

