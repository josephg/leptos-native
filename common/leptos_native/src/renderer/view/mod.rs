//! Core view-tree traits — `Render<R>`, `Mountable<R>`, `IntoRender<R>`.
//!
//! Each platform implements a `Backend` (see `crate::renderer::Backend`).
//! View types implement `Render<R>` to describe how they build platform
//! state, and the resulting `State` impls `Mountable<R>` so it can be
//! attached to / detached from a parent.

use crate::renderer::node::Node;
use crate::renderer::Backend;

mod add_any_attr;
mod any_view;
mod either;
mod iterators;
mod keyed;
mod option;
mod primitives;
mod result;
mod strings;
mod tuples;

pub use keyed::{keyed, Keyed, KeyedState};

pub use add_any_attr::{AddAnyAttr, ApplyAttr};

pub use any_view::{AnyView, AnyViewState, IntoAny};

pub use iterators::VecState;
pub use tuples::UnitState;

/// Allows rendering some value as part of the user interface.
///
/// Each value `V: Render<R>` produces a `State` that owns the live UI nodes;
/// `rebuild` updates them in place when the value changes (driven by the
/// reactive graph).
pub trait Render<R: Backend>: Sized {
    /// Retained between updates. Typically owns the actual platform nodes
    /// for this view, plus enough information to compute a diff against
    /// new data.
    type State: Mountable<R>;

    /// Builds the view for the first time. Nodes are allocated into the
    /// ambient thread-local store (see [`crate::scene::Backend::with_tree`]).
    fn build(self) -> Self::State;

    /// Updates the view with new data, in place.
    fn rebuild(self, _state: &mut Self::State) {}
}

/// Allows a view's state to be attached to / detached from the platform tree.
pub trait Mountable<R: Backend> {
    /// Detaches this view from its parent.
    fn unmount(&mut self);

    /// Mounts this view under `parent`. If `marker` is `Some`, inserts
    /// before the marker; otherwise appends.
    fn mount(&mut self, parent: Node<R>, marker: Option<Node<R>>);

    /// Same as `mount`, but returns `false` if it could not mount.
    fn try_mount(
        &mut self,
        parent: Node<R>,
        marker: Option<Node<R>>,
    ) -> bool {
        self.mount(parent, marker);
        true
    }

    /// Inserts another mountable before this one. Returns `false` if this
    /// view has no real presence in the UI to splice before.
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool;

    /// Inserts `child` before this view, or before `marker` if this view
    /// has no real presence in the UI.
    fn insert_before_this_or_marker(
        &self,
        parent: Node<R>,
        child: &mut dyn Mountable<R>,
        marker: Option<Node<R>>,
    ) {
        if !self.insert_before_this(child) {
            child.mount(parent, marker);
        }
    }

    // I'd like to have this return an element iterator to prevent a lot of allocations,
    // but this type needs to be able to go in Box<dyn _> for any_view.
    // type ElemIter: Iterator<Item=Node<R>>;
    /// Returns the elements owned by this view (used for things like NodeRef
    /// resolution).
    fn elements(&self) -> Vec<Node<R>>;
}

/// Declares that this type can be converted into something that can be rendered.
pub trait IntoRender<R: Backend> {
    /// The output of the conversion.
    type Output;

    /// Performs the conversion.
    fn into_render(self) -> Self::Output;
}

impl<R: Backend, T: Render<R>> IntoRender<R> for T {
    type Output = Self;

    fn into_render(self) -> Self::Output {
        self
    }
}
