//! The [`Renderer`] trait — the interface each platform implements to provide
//! concrete `Element`/`Node`/`Text`/`Placeholder` types and the imperative
//! operations (create, set_attribute, insert_node, etc.) the view tree calls.

use crate::layout::LayoutBackend;
use crate::view::Mountable;
use std::fmt::Debug;

/// Implements the instructions necessary to render an interface on some
/// platform. Each platform supplies its own `Renderer` impl.
pub trait Renderer: Send + Sized + Debug + 'static {
    /// Per-platform layout backend. `Render::build` takes a
    /// `&TreeRef<Self::Backend>` so each builder can allocate its
    /// arena entry into the correct window's tree without a hidden
    /// thread-local. Cocoa sets this to `CocoaBackend`, GTK to
    /// `GtkBackend`, iOS to `IosBackend`.
    type Backend: LayoutBackend;

    /// The basic type of node in the view tree.
    ///
    /// Native ports collapse the old web-DOM `Element` / `Text` /
    /// `Placeholder` associated types into a single `Node` — every
    /// arena entry is structurally Element-shaped, and text-label /
    /// placeholder distinctions are just different default styles +
    /// concrete view classes set at construction time. Builder code
    /// that wants a Node back from a `&Node` (e.g. for `mount_before`)
    /// uses [`CastFrom::cast_from`].
    type Node: AsRef<Self::Node>
        + CastFrom<Self::Node>
        + Mountable<Self>
        + Clone
        + 'static;

    /// Interns a string slice, if that's available on this platform and
    /// useful as an optimization.
    fn intern(text: &str) -> &str {
        text
    }

    /// Creates a new text node in the given layout tree.
    fn create_text_node(tree: &crate::TreeRef<Self::Backend>, text: &str) -> Self::Node;

    /// Creates a new placeholder node in the given layout tree.
    fn create_placeholder(tree: &crate::TreeRef<Self::Backend>) -> Self::Node;

    /// Sets the text content of a text node.
    fn set_text(node: &Self::Node, text: &str);
    
    /// Inserts `new_child` into `parent` before `marker`. If `marker` is
    /// `None`, appends to the end.
    fn insert_node(
        parent: &Self::Node,
        new_child: &Self::Node,
        marker: Option<&Self::Node>,
    );

    /// Removes `child` from `parent` and returns it.
    fn remove_node(
        parent: &Self::Node,
        child: &Self::Node,
    ) -> Option<Self::Node>;

    /// Removes all children from `parent`.
    fn clear_children(parent: &Self::Node);

    /// Removes a node from its parent.
    fn remove(node: &Self::Node);

    /// Gets the parent of a node, if any.
    fn get_parent(node: &Self::Node) -> Option<Self::Node>;

    /// Returns the first child of a node, if any.
    fn first_child(node: &Self::Node) -> Option<Self::Node>;

    /// Returns the next sibling of a node, if any.
    fn next_sibling(node: &Self::Node) -> Option<Self::Node>;

    /// Logs a node in a platform-appropriate way (used for debugging).
    fn log_node(node: &Self::Node);

    /// Mounts `new_child` into the parent of `before`, immediately before
    /// `before`. Returns `false` if `before` has no parent (in which case
    /// the caller is responsible for finding a different mount point).
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: &Self::Node) -> bool
    where
        M: Mountable<Self>,
    {
        if let Some(parent) =
            Self::get_parent(before).and_then(Self::Node::cast_from)
        {
            new_child.mount(&parent, Some(before));
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
