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
    pub use super::{
        attrs::{
            AlignSelf, DecorationAttrs, Dim, IntoMaybeReactive, LayoutAttrs,
            MaybeReactive, TextAttrs, UniversalAttrs, WithDecoration,
            WithLayout, WithText, WithUniversal,
        },
        Renderer,
        view::{AddAnyAttr, IntoRender, Mountable, Render},
    };
}

/// Cross-backend attribute plumbing: `MaybeReactive`, `Dim`,
/// `WithLayout` / `WithUniversal` / `WithText` traits, and the
/// `LayoutAttrs` / `UniversalAttrs` / `TextAttrs` structs each
/// builder embeds.
pub mod attrs;

/// Core logic for manipulating views (`Render`, `Mountable`, view impls for
/// tuples, primitives, strings, iterators, either, fragments, keyed lists).
pub mod view;

/// The renderer-agnostic view handle [`node::Node<B>`] — a `Copy + Send`
/// generational id tagged with its backend. Replaces the per-port
/// `*Elem` newtypes; ports add platform widget setters via an extension
/// trait `impl … for Node<PortBackend>`.
pub mod node;

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

use std::fmt::Debug;
// Re-export every scene + setters item at the renderer root so
// consumer paths (`use renderer::{Style, set_padding, LayoutNodeOps}`)
// match the shape the per-port code already uses.
pub use node::Node;
pub use scene::*;
pub use setters::*;
pub use window::{WindowPosition, WindowSize};

pub use either_of as either;
use crate::renderer::prelude::Mountable;

/// View implementations for the `reactive_graph` crate (closures as reactive
/// children, signals as reactive attribute values).
#[cfg(feature = "reactive_graph")]
pub mod reactive_graph;


/// Implements the instructions necessary to render an interface on some
/// platform. Each platform supplies its own `Renderer` impl.
pub trait Renderer: Send + Sized + Debug + 'static {
    /// Per-platform layout backend. The node store is a thread-local
    /// singleton reached via [`LayoutBackend::with_tree`], so `build`
    /// takes no tree handle. Cocoa sets this to `CocoaBackend`, GTK to
    /// `GtkBackend`, iOS to `IosBackend`.
    type Backend: LayoutBackend;

    /// The basic type of node in the view tree. Native ports wrap a
    /// bare `NodeId` (`Copy + Send`) — every entry is structurally
    /// Element-shaped, and text-label / placeholder distinctions are
    /// just different default styles + concrete view classes set at
    /// construction time. Stale ids resolve to no-ops via the
    /// generational store key.
    type Node: Mountable<Self> + Clone + Copy + 'static;

    /// Interns a string slice, if that's available on this platform and
    /// useful as an optimization.
    fn intern(text: &str) -> &str {
        text
    }

    /// Creates a new text node in the ambient node store.
    fn create_text_node(text: &str) -> Self::Node;

    /// Creates a new placeholder node in the ambient node store.
    fn create_placeholder() -> Self::Node;

    /// Sets the text content of a text node.
    fn set_text(node: Self::Node, text: &str);

    /// Inserts `new_child` into `parent` before `marker`. If `marker` is
    /// `None`, appends to the end.
    fn insert_node(
        parent: Self::Node,
        new_child: Self::Node,
        marker: Option<Self::Node>,
    );

    /// Removes `child` from `parent` and returns it.
    fn remove_node(
        parent: Self::Node,
        child: Self::Node,
    ) -> Option<Self::Node>;

    /// Removes all children from `parent`.
    fn clear_children(parent: Self::Node);

    /// Removes a node from its parent.
    fn remove(node: Self::Node);

    /// Gets the parent of a node, if any.
    fn get_parent(node: Self::Node) -> Option<Self::Node>;

    /// Logs a node in a platform-appropriate way (used for debugging).
    fn log_node(node: Self::Node);

    /// Mounts `new_child` into the parent of `before`, immediately before
    /// `before`. Returns `false` if `before` has no parent (in which case
    /// the caller is responsible for finding a different mount point).
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: Self::Node) -> bool
    where
        M: Mountable<Self>,
    {
        if let Some(parent) = Self::get_parent(before) {
            new_child.mount(parent, Some(before));
            true
        } else {
            false
        }
    }
}
