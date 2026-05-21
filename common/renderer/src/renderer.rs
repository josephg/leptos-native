//! The [`Renderer`] trait — the interface each platform implements to provide
//! concrete `Element`/`Node`/`Text`/`Placeholder` types and the imperative
//! operations (create, set_attribute, insert_node, etc.) the view tree calls.

use crate::scene::LayoutBackend;
use crate::view::Mountable;
use std::fmt::Debug;

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

/// Attempts to cast from one type to another.
///
/// Like `TryFrom`, but as a separate trait so it can be implemented on
/// foreign types without orphan-rule issues.
pub trait CastFrom<T>
where
    Self: Sized,
{
    /// Casts a node from one type to another.
    fn cast_from(source: T) -> Option<Self>;
}
