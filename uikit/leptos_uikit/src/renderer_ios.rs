//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] impls on the ios_dom types.
//!
//! Mirror of `cocoa/leptos_cocoa/src/renderer_cocoa.rs`. CastFrom impls
//! live in `ios_dom::renderer` (orphan rule — see comment there).

#![allow(missing_docs)]

use ios_dom::Renderer as IosRenderer;
use renderer::{
    renderer::Renderer as RendererTrait,
    view::Mountable,
};
use ios_dom::layout::IosBackend;
use renderer::LayoutBackend;

// `Text` and `Placeholder` are aliases for `Element` — the renderer
// trait wants distinct associated types, but on native they're all
// UIView-backed Elements; the only thing distinguishing a "text node"
// or "placeholder" from a regular Element is the UIView subclass +
// default style applied at creation.
pub use ios_dom::{
    ClassList, CssStyleDeclaration, Element, Event, Node, TemplateElement,
};
pub type Text = Element;
pub type Placeholder = Element;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Backend = ios_dom::layout::IosBackend;
    type Node = Node;

    fn intern(text: &str) -> &str {
        IosRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> Element {
        IosRenderer::create_text_node(text)
    }

    fn create_placeholder() -> Element {
        IosRenderer::create_placeholder()
    }

    fn set_text(node: &Element, text: &str) {
        IosRenderer::set_text(node, text);
    }

    fn insert_node(
        parent: &Element,
        new_child: &Node,
        anchor: Option<&Node>,
    ) {
        IosRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        IosRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: &Element) {
        IosRenderer::clear_children(parent);
    }

    fn remove(node: &Node) {
        IosRenderer::remove(node);
    }

    // get_parent / first_child / next_sibling: cocoa returns None to
    // dodge ios_dom's "hydration not supported" panics; same here.
    fn get_parent(_node: &Node) -> Option<Node> {
        None
    }

    fn first_child(_node: &Node) -> Option<Node> {
        None
    }

    fn next_sibling(_node: &Node) -> Option<Node> {
        None
    }

    fn log_node(node: &Node) {
        IosRenderer::log_node(node);
    }

    /// Override the trait default: synthesise a parent Element from
    /// `before`'s superview, with the right LayoutHandle so the new
    /// child registers in the same Taffy tree.
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
    /// `before` has no parent (must-succeed variant).
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
/// root. No view-wrapper synthesis needed under the thread-local store.
fn parent_of(before: &Node) -> Option<Node> {
    IosBackend::parent(before.id())
        .map(Node::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls
// ---------------------------------------------------------------------

impl Mountable<Dom> for Node {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        <Dom as RendererTrait>::insert_node(parent, self, marker);
    }

    fn try_mount(
        &mut self,
        parent: &Element,
        marker: Option<&Node>,
    ) -> bool {
        IosRenderer::try_insert_node(parent, self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<Element> {
        vec![self.clone()]
    }
}

