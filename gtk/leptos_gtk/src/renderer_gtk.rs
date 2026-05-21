//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<GtkDom>`] / [`CastFrom`] impls that sit on the gtk_dom
//! types.
//!
//! Mirror of `leptos_cocoa::renderer_cocoa`. `Dom` is a unit struct
//! (not a type alias for `gtk_dom::Renderer`) so we can attach trait
//! impls and method extensions here without orphan-rule grief, and so
//! callers can write `<Dom as Renderer>::*` exactly as the
//! renderer-agnostic core (e.g. `common/renderer/src/view/iterators.rs`)
//! calls it.

#![allow(missing_docs)]

use crate::dom::layout;
use renderer::{
    renderer::Renderer,
    view::Mountable,
};

// Re-export the concrete tree types under the names the platform
// expects. `Text` and `Placeholder` are aliases for `Element` — the
// renderer trait wants distinct associated types, but on native
// they're all widget-backed Elements; the only thing distinguishing
// a "text node" or "placeholder" from a regular Element is the
// widget subclass + default style applied at creation.
pub use crate::dom::{
    Event, GtkNode,
};
pub type Text = GtkNode;
pub type Placeholder = GtkNode;
use renderer::scene::LayoutBackend;

/// The GTK renderer surface — implements [`renderer::Renderer`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GtkDom;

impl Renderer for GtkDom {
    type Backend = layout::GtkBackend;
    type Node = GtkNode;

    fn intern(text: &str) -> &str {
        text
    }

    fn create_text_node(text: &str) -> GtkNode {
        GtkNode::create_text(text)
    }

    fn create_placeholder() -> GtkNode {
        GtkNode::create_placeholder()
    }

    fn set_text(node: GtkNode, text: &str) {
        node.set_text(text);
    }

    fn insert_node(
        parent: GtkNode,
        new_child: GtkNode,
        anchor: Option<GtkNode>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    fn remove_node(parent: GtkNode, child: GtkNode) -> Option<GtkNode> {
        parent.remove_child(child)
    }

    fn clear_children(parent: GtkNode) {
        parent.clear_children();
    }

    fn remove(node: GtkNode) {
        layout::drop_node(node);
    }

    fn get_parent(node: GtkNode) -> Option<GtkNode> {
        parent_of(node)
    }

    fn log_node(node: GtkNode) {
        eprintln!("[gtk_dom] {node:?}");
    }
}

impl GtkDom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no parent (mirror of `try_mount_before`).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: GtkNode)
    where
        M: Mountable<GtkDom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(parent, Some(before));
    }
}

/// The parent `Node` of `before` in the store, or `None` if it's a
/// root. The parent is a real node — no widget-wrapper synthesis is
/// needed under the thread-local store.
fn parent_of(before: GtkNode) -> Option<GtkNode> {
    crate::dom::layout::GtkBackend::parent(before.id())
        .map(GtkNode::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these live in this crate.
// ---------------------------------------------------------------------

impl Mountable<GtkDom> for GtkNode {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: GtkNode, marker: Option<GtkNode>) {
        <GtkDom as Renderer>::insert_node(parent, *self, marker);
    }

    fn try_mount(
        &mut self,
        parent: GtkNode,
        marker: Option<GtkNode>,
    ) -> bool {
        parent.try_insert_node(*self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<GtkDom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkNode> {
        vec![self.clone()]
    }
}

