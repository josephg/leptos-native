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
        renderer::Renderer,
        view::{AddAnyAttr, IntoRender, Mountable, Render},
    };
}

/// Defines the [`Renderer`](renderer::Renderer) trait — the interface each
/// platform implements to provide concrete `Element`/`Node`/`Text`/
/// `Placeholder` types.
pub mod renderer;
/// Core logic for manipulating views (`Render`, `Mountable`, view impls for
/// tuples, primitives, strings, iterators, either, fragments, keyed lists).
pub mod view;

pub use either_of as either;

/// View implementations for the `reactive_graph` crate (closures as reactive
/// children, signals as reactive attribute values).
#[cfg(feature = "reactive_graph")]
pub mod reactive_graph;

