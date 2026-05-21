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

use crate::dom::Renderer as GtkRenderer;
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
pub use crate::dom::{
    ClassList, CssStyleDeclaration, Event, GtkNode, TemplateElement,
};
pub type Text = GtkNode;
pub type Placeholder = GtkNode;
use renderer::scene::LayoutBackend;

/// The GTK renderer surface — implements [`renderer::Renderer`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Backend = crate::dom::layout::GtkBackend;
    type Node = GtkNode;

    fn intern(text: &str) -> &str {
        GtkRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> GtkNode {
        GtkRenderer::create_text_node(text)
    }

    fn create_placeholder() -> GtkNode {
        GtkRenderer::create_placeholder()
    }

    fn set_text(node: &GtkNode, text: &str) {
        GtkRenderer::set_text(node, text);
    }

    fn insert_node(
        parent: &GtkNode,
        new_child: &GtkNode,
        anchor: Option<&GtkNode>,
    ) {
        GtkRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: &GtkNode, child: &GtkNode) -> Option<GtkNode> {
        GtkRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: &GtkNode) {
        GtkRenderer::clear_children(parent);
    }

    fn remove(node: &GtkNode) {
        GtkRenderer::remove(node);
    }

    fn get_parent(node: &GtkNode) -> Option<GtkNode> {
        // The default `try_mount_before` impl on the trait calls
        // get_parent. gtk_dom's get_parent panics with a hydration
        // message; here we return None so try_mount_before falls back
        // to our overridden version below.
        let _ = node;
        None
    }

    fn first_child(node: &GtkNode) -> Option<GtkNode> {
        let _ = node;
        None
    }

    fn next_sibling(node: &GtkNode) -> Option<GtkNode> {
        let _ = node;
        None
    }

    fn log_node(node: &GtkNode) {
        GtkRenderer::log_node(node);
    }

    /// Override the trait's default — on GTK we synthesise a parent
    /// Element wrapper around `before`'s GTK parent, with the right
    /// LayoutHandle so the new child registers in the same Taffy
    /// tree.
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: &GtkNode) -> bool
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
    pub fn mount_before<M>(new_child: &mut M, before: &GtkNode)
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
fn parent_of(before: &GtkNode) -> Option<GtkNode> {
    crate::dom::layout::GtkBackend::parent(before.id())
        .map(GtkNode::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these live in this crate.
// ---------------------------------------------------------------------

impl Mountable<Dom> for GtkNode {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: &GtkNode, marker: Option<&GtkNode>) {
        <Dom as RendererTrait>::insert_node(parent, self, marker);
    }

    fn try_mount(
        &mut self,
        parent: &GtkNode,
        marker: Option<&GtkNode>,
    ) -> bool {
        GtkRenderer::try_insert_node(parent, self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkNode> {
        vec![self.clone()]
    }
}

