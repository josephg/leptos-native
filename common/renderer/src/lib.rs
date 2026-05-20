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
            AlignSelf, DecorationAttrs, Dim, IntoMaybeReactive, LayoutAttrs,
            MaybeReactive, TextAttrs, UniversalAttrs, WithDecoration,
            WithLayout, WithText, WithUniversal,
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

/// The retained render tree: a per-thread [`scene::LayoutState<B>`]
/// node store (generational slotmap of view + style + handlers),
/// generic over a [`LayoutBackend`](scene::LayoutBackend), with the
/// Taffy layout engine, the `NodeId` free-fn API, the per-port `Style`
/// re-exports, and the grid track-sizing helpers. Lives here (not its
/// own crate) so [`setters`]'s `IntoMaybeReactive` impls for taffy
/// types satisfy the orphan rule.
pub mod scene;

/// Generic, port-agnostic style mutators (`set_padding`,
/// `set_grid_template_columns`, …) and the trait-driven
/// `apply_layout` / `apply_universal` install loops. Each port impls
/// [`setters::LayoutNodeOps`] / [`setters::LayoutElement`] /
/// [`setters::UniversalElement`] for its node / element types and
/// reuses the generic functions.
pub mod setters;

/// Cross-backend menu types. Currently just [`menu::Modifiers`] —
/// each port translates it to its platform-native modifier shape
/// (`NSEventModifierFlags` on AppKit, GTK's `<Primary><Shift>` accel
/// strings on GTK).
pub mod menu;

/// Shared `WindowSize` / `WindowPosition` newtypes with
/// `From<(i32, i32)>` and `From<(f64, f64)>` impls.
pub mod window;

/// `use:directive=param` plumbing — the `IntoDirective` trait
/// plus `pack` / `run_all` helpers for builders' directive Vec.
/// Generic over element type; each port re-exports binding the
/// generic `E` to its own `Element`.
pub mod directive;

/// Attribute-key marker types used by the `bind:` machinery in
/// each port (`Value`, `Checked`, `AttributeKey`). Used solely to
/// disambiguate `BindAttribute<Key, Sig>` impls per-control; never
/// read or written as a real element attribute.
pub mod attr_keys;

// Re-export every scene + setters item at the renderer root so
// consumer paths (`use renderer::{Style, set_padding, LayoutNodeOps}`)
// match the shape the per-port code already uses.
pub use scene::*;
pub use setters::*;
pub use window::{WindowPosition, WindowSize};

pub use either_of as either;

/// View implementations for the `reactive_graph` crate (closures as reactive
/// children, signals as reactive attribute values).
#[cfg(feature = "reactive_graph")]
pub mod reactive_graph;

