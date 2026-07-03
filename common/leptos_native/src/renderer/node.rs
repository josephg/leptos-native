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

use super::scene::{AttachOutcome, Backend, NodeId, Style};
use super::view::Mountable;

/// A `Copy` handle into the ambient [`LayoutState<B>`] node store —
/// structurally just a [`NodeId`]. See the module docs.
pub struct Node<B: Backend> {
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
impl<B: Backend> Clone for Node<B> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<B: Backend> Copy for Node<B> {}
impl<B: Backend> PartialEq for Node<B> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl<B: Backend> Eq for Node<B> {}
impl<B: Backend> Hash for Node<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl<B: Backend> fmt::Debug for Node<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Node").field(&self.id).finish()
    }
}

impl<B: Backend> Node<B> {
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

    /// Read the node's Taffy [`Style`] in place (no clone). If the node
    /// is gone, `f` sees a default style. The closure runs while the
    /// store is borrowed — it must not re-enter the store (read the
    /// fields you need and return them).
    pub fn with_style<R>(self, f: impl FnOnce(&Style) -> R) -> R {
        B::with_tree(|s| match s.style_ref(self.id) {
            Some(style) => f(style),
            None => f(&Style::default()),
        })
    }

    /// Mutate the node's Taffy [`Style`] in place (marks dirty). A
    /// stale handle mutates a scratch style (no-op). Same re-entrancy
    /// rule as [`Self::with_style`].
    pub fn with_style_mut<R>(self, f: impl FnOnce(&mut Style) -> R) -> R {
        B::with_tree(|s| s.update_style(self.id, f))
    }

    /// Read the node's backend metadata in place. A stale handle sees
    /// the default. Same re-entrancy rule as [`Self::with_style`].
    pub fn with_meta<R>(self, f: impl FnOnce(&B::NodeMeta) -> R) -> R {
        B::with_tree(|s| match s.meta_ref(self.id) {
            Some(meta) => f(meta),
            None => f(&B::NodeMeta::default()),
        })
    }

    /// Mutate the node's backend metadata in place. A stale handle
    /// mutates a scratch value (no-op). Same re-entrancy rule as
    /// [`Self::with_style`].
    pub fn with_meta_mut<R>(self, f: impl FnOnce(&mut B::NodeMeta) -> R) -> R {
        B::with_tree(|s| s.update_meta(self.id, f))
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

    // ---- tree edits: native (via the backend) + Taffy mirror, once ----

    /// Insert `child` under `self` immediately before `marker` (append if
    /// `None`). Drives the port's native attach, then mirrors the edge
    /// into Taffy by marker. Returns `false` if the insert was rejected
    /// (self-parent / marker isn't a child / unsupported parent).
    pub fn insert_node(self, child: Self, marker: Option<Self>) -> bool {
        match B::attach_native(self.id, child.id, marker.map(|m| m.id)) {
            AttachOutcome::Mirror => {
                B::with_tree(|s| s.insert_child_before(self.id, child.id, marker.map(|m| m.id)));
                // Symmetric with `remove()`: a structural edit changes
                // layout, so queue a (deduped) relayout for the parent.
                // Without this, a pure reorder (`<For>` keyed move with no
                // add/remove and no text change — e.g. shuffling rows) marks
                // Taffy dirty but never dispatches a compute pass, so the
                // reorder stays invisible until an unrelated layout trigger.
                B::schedule_relayout(self.id);
                true
            }
            AttachOutcome::NativeOnly => {
                B::schedule_relayout(self.id);
                true
            }
            AttachOutcome::Rejected => false,
        }
    }

    /// Detach `child` from `self` (native + Taffy). Returns `Some(child)`
    /// if it was actually a child, else `None`. Does **not** free `child`.
    pub fn remove_child(self, child: Self) -> Option<Self> {
        if B::detach_native(self.id, child.id) {
            B::with_tree(|s| s.remove_child(self.id, child.id));
            Some(child)
        } else {
            None
        }
    }

    /// Remove every child of `self` (native level). Taffy children are
    /// reclaimed by the remove cascade, matching prior per-port behavior.
    pub fn clear_children(self) {
        B::clear_native_children(self.id);
    }

    /// The node's parent in the store, if any.
    pub fn parent(self) -> Option<Self> {
        B::parent(self.id).map(Node::from_id)
    }

    /// Mark this node dirty and queue a (deduped) relayout pass for its
    /// tree on the next main-loop tick.
    pub fn schedule_relayout(self) {
        B::schedule_relayout(self.id);
    }

    /// Tear the node down: detach its view from the native parent, free
    /// the store entry (cascading through the structural subtree), and
    /// queue a relayout for the ex-parent so siblings reflow. Idempotent —
    /// a stale handle is a no-op.
    pub fn remove(self) {
        let parent = B::parent(self.id);
        if let Some(view) = self.try_view() {
            B::remove_from_native_parent(&view);
        }
        B::remove(self.id);
        if let Some(pid) = parent {
            B::schedule_relayout(pid);
        }
    }
}

// ---------------------------------------------------------------------
// Mountable — how the view-tree core attaches/detaches a node. One
// blanket impl; mounting is fully expressible through the backend's
// native-edit hooks, so ports don't write this per-port any more.
// ---------------------------------------------------------------------

impl<B: Backend> Mountable<B> for Node<B> {
    fn unmount(&mut self) {
        (*self).remove();
    }

    fn mount(&mut self, parent: Self, marker: Option<Self>) {
        parent.insert_node(*self, marker);
    }

    fn try_mount(&mut self, parent: Self, marker: Option<Self>) -> bool {
        parent.insert_node(*self, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<B>) -> bool {
        let Some(parent) = self.parent() else {
            return false;
        };
        child.mount(parent, Some(*self));
        true
    }

    fn elements(&self) -> Vec<Self> {
        vec![*self]
    }
}
