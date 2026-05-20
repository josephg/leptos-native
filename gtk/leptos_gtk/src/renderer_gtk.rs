//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] / [`CastFrom`] impls that sit on the gtk_dom
//! types.
//!
//! Mirror of `leptos_cocoa::renderer_cocoa`. `Dom` is a unit struct
//! (not a type alias for `gtk_dom::Renderer`) so we can attach trait
//! impls and method extensions here without orphan-rule grief, and so
//! callers can write `<Dom as Renderer>::*` exactly as the
//! renderer-agnostic core (e.g. `common/renderer/src/view/iterators.rs`)
//! calls it.

#![allow(missing_docs)]

use gtk_dom::Renderer as GtkRenderer;
use renderer::{
    renderer::Renderer as RendererTrait,
    view::Mountable,
};

// Re-export the concrete tree types under the names the platform
// expects. `Text` and `Placeholder` are aliases for `Element` — the
// renderer trait wants distinct associated types, but on native
// they're all widget-backed Elements; the only thing distinguishing
// a "text node" or "placeholder" from a regular Element is the
// widget subclass + default style applied at creation.
pub use gtk_dom::{
    ClassList, CssStyleDeclaration, Event, Node, TemplateElement,
};
pub type Text = Node;
pub type Placeholder = Node;

/// The GTK renderer surface — implements [`renderer::Renderer`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Backend = gtk_dom::layout::GtkBackend;
    type Node = Node;

    fn intern(text: &str) -> &str {
        GtkRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> Node {
        GtkRenderer::create_text_node(text)
    }

    fn create_placeholder() -> Node {
        GtkRenderer::create_placeholder()
    }

    fn set_text(node: &Node, text: &str) {
        GtkRenderer::set_text(node, text);
    }

    fn insert_node(
        parent: &Node,
        new_child: &Node,
        anchor: Option<&Node>,
    ) {
        GtkRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: &Node, child: &Node) -> Option<Node> {
        GtkRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: &Node) {
        GtkRenderer::clear_children(parent);
    }

    fn remove(node: &Node) {
        GtkRenderer::remove(node);
    }

    fn get_parent(node: &Node) -> Option<Node> {
        // The default `try_mount_before` impl on the trait calls
        // get_parent. gtk_dom's get_parent panics with a hydration
        // message; here we return None so try_mount_before falls back
        // to our overridden version below.
        let _ = node;
        None
    }

    fn first_child(node: &Node) -> Option<Node> {
        let _ = node;
        None
    }

    fn next_sibling(node: &Node) -> Option<Node> {
        let _ = node;
        None
    }

    fn log_node(node: &Node) {
        GtkRenderer::log_node(node);
    }

    /// Override the trait's default — on GTK we synthesise a parent
    /// Element wrapper around `before`'s GTK parent, with the right
    /// LayoutHandle so the new child registers in the same Taffy
    /// tree.
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: &Node) -> bool
    where
        M: Mountable<Self>,
    {
        let Some(parent) = parent_of(before) else {
            return false;
        };
        new_child.mount(&parent, Some(before));
        true
    }
}

impl Dom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no parent (mirror of `try_mount_before`).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: &Node)
    where
        M: Mountable<Dom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(&parent, Some(before));
    }
}

/// The parent `Node` of `before` in the store, or `None` if it's a
/// root. The parent is a real node — no widget-wrapper synthesis is
/// needed under the thread-local store.
fn parent_of(before: &Node) -> Option<Node> {
    renderer::parent::<gtk_dom::layout::GtkBackend>(before.id())
        .map(Node::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these live in this crate.
// ---------------------------------------------------------------------

impl Mountable<Dom> for Node {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: &Node, marker: Option<&Node>) {
        <Dom as RendererTrait>::insert_node(parent, self, marker);
    }

    fn try_mount(
        &mut self,
        parent: &Node,
        marker: Option<&Node>,
    ) -> bool {
        GtkRenderer::try_insert_node(parent, self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<Node> {
        vec![self.clone()]
    }
}

