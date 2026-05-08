//! The [`Renderer`] trait — the interface each platform implements to provide
//! concrete `Element`/`Node`/`Text`/`Placeholder` types and the imperative
//! operations (create, set_attribute, insert_node, etc.) the view tree calls.

use crate::view::Mountable;
use std::fmt::Debug;

/// Implements the instructions necessary to render an interface on some
/// platform. Each platform supplies its own `Renderer` impl.
pub trait Renderer: Send + Sized + Debug + 'static {
    /// The basic type of node in the view tree.
    type Node: Mountable<Self> + Clone + 'static;
    /// A visible element in the view tree.
    type Element: AsRef<Self::Node>
        + CastFrom<Self::Node>
        + Mountable<Self>
        + Clone
        + 'static;
    /// A text node in the view tree.
    type Text: AsRef<Self::Node>
        + CastFrom<Self::Node>
        + Mountable<Self>
        + Clone
        + 'static;
    /// A placeholder node, which can be inserted into the tree but does not
    /// appear (e.g., a comment node in the DOM).
    type Placeholder: AsRef<Self::Node>
        + CastFrom<Self::Node>
        + Mountable<Self>
        + Clone
        + 'static;

    /// Interns a string slice, if that's available on this platform and
    /// useful as an optimization.
    fn intern(text: &str) -> &str {
        text
    }

    /// Creates a new text node.
    fn create_text_node(text: &str) -> Self::Text;

    /// Creates a new placeholder node.
    fn create_placeholder() -> Self::Placeholder;

    /// Sets the text content of a text node.
    fn set_text(node: &Self::Text, text: &str);

    /// Sets the given attribute on the given element by key and value.
    fn set_attribute(node: &Self::Element, name: &str, value: &str);

    /// Removes the given attribute from the given element.
    fn remove_attribute(node: &Self::Element, name: &str);

    /// Inserts `new_child` into `parent` before `marker`. If `marker` is
    /// `None`, appends to the end.
    fn insert_node(
        parent: &Self::Element,
        new_child: &Self::Node,
        marker: Option<&Self::Node>,
    );

    /// Removes `child` from `parent` and returns it.
    fn remove_node(
        parent: &Self::Element,
        child: &Self::Node,
    ) -> Option<Self::Node>;

    /// Removes all children from `parent`.
    fn clear_children(parent: &Self::Element);

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
            Self::get_parent(before).and_then(Self::Element::cast_from)
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
