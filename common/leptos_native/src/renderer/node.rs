//! [`Node<B>`] — the renderer-agnostic view handle.
//!
//! A `Node<B>` is a bare generational [`NodeId`] tagged with its backend
//! `B`. It is `Copy + Send + Sync` and holds **no** per-node state: the
//! platform view (`Retained<NSView>`, `gtk::Widget`, …) and Taffy style
//! live in the thread-local [`LayoutState<B>`](super::scene::LayoutState),
//! reached by id. A stale id resolves to `None`/no-op via the generational
//! slotmap key, giving weak-reference behaviour for free.
//!
//! This replaces the per-port `CocoaElem` / `GtkElem` / `UikitElem`
//! newtypes, which were byte-identical (`struct XElem { id: NodeId }`) and
//! re-declared the same generic accessor surface. That surface now lives
//! here once. **Platform-specific** widget setters (`set_title`,
//! `create_button`, the `on_*` installers, two-way bind helpers) cannot be
//! inherent methods on `Node<B>` from a port crate — inherent impls must
//! live in the defining crate — so each port supplies them via its own
//! extension trait `impl … for Node<PortBackend>` (orphan-rule-safe: the
//! trait is the port's, the backend type is the port's).
//!
//! `Send`/`Sync` do not depend on `B`: the marker is `PhantomData<fn() ->
//! B>` (a function pointer is unconditionally `Send + Sync + Copy`), so a
//! backend whose `View` is `!Send` (every real port) still yields a `Send`
//! handle — exactly the property that lets the handle cross the generic
//! plumbing while the `!Send` view stays pinned in the thread-local arena.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use super::scene::{LayoutBackend, NodeId, Style};

/// A `Copy` handle into the ambient [`LayoutState<B>`] node store —
/// structurally just a [`NodeId`]. See the module docs.
pub struct Node<B: LayoutBackend> {
    /// The backing store key. Readable (it's the whole handle), but the
    /// private `_b` marker keeps `Node` non-constructible outside this
    /// module — use [`Node::from_id`]. Ports' extension-trait method
    /// bodies read `self.id` directly.
    pub id: NodeId,
    _b: PhantomData<fn() -> B>,
}

// Hand-written rather than derived: `derive` would spuriously require
// `B: Clone/Copy/PartialEq/…`, but the handle's traits depend only on
// `NodeId` (and `PhantomData<fn() -> B>`, which is unconditional).
impl<B: LayoutBackend> Clone for Node<B> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<B: LayoutBackend> Copy for Node<B> {}
impl<B: LayoutBackend> PartialEq for Node<B> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<B: LayoutBackend> Eq for Node<B> {}
impl<B: LayoutBackend> Hash for Node<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl<B: LayoutBackend> fmt::Debug for Node<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Node").field(&self.id).finish()
    }
}

impl<B: LayoutBackend> Node<B> {
    /// Wrap an existing store id as a handle. (No store access — cheap.)
    pub fn from_id(id: NodeId) -> Self {
        Node { id, _b: PhantomData }
    }

    /// The node's [`NodeId`].
    pub fn id(self) -> NodeId {
        self.id
    }

    /// The node's platform view (cheap clone — a gobject/ObjC refcount
    /// bump). Main-thread only. Panics if the node is gone; use
    /// [`Self::try_view`] for the graceful variant.
    pub fn view(self) -> B::View {
        B::view(self.id).expect("Node id must exist in the store")
    }

    /// `Some(view)` if the node is still in the store, else `None`.
    ///
    /// Setters resolve through this (not the panicking [`Self::view`]) so a
    /// reactive effect that fires *after* the node was torn down is a
    /// graceful no-op: a `RenderEffect` closure captures only the `id`, so
    /// an async-scheduled re-run can outlive its node.
    pub fn try_view(self) -> Option<B::View> {
        B::view(self.id)
    }

    /// Read the node's Taffy [`Style`] (clone-and-borrow).
    pub fn with_style<R>(self, f: impl FnOnce(&Style) -> R) -> R {
        let style = B::style(self.id).unwrap_or_default();
        f(&style)
    }

    /// Mutate the node's Taffy [`Style`] and write it back (marks dirty).
    pub fn with_style_mut<R>(self, f: impl FnOnce(&mut Style) -> R) -> R {
        let mut style = B::style(self.id).unwrap_or_default();
        let r = f(&mut style);
        B::set_style(self.id, style);
        r
    }

    /// Record the element kind for debug tooling (devtools inspector).
    /// Cheap; set once at construction. Returns `self` for chaining.
    pub fn with_tag(self, tag: &'static str) -> Self {
        B::set_debug_tag_name(self.id, tag);
        self
    }

    /// Pointer-equality: each node owns one view, so id equality is
    /// underlying-view equality.
    pub fn ptr_eq(self, other: Self) -> bool {
        self.id == other.id
    }
}
