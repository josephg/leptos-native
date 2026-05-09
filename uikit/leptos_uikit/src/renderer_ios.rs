//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] impls on the ios_dom types.
//!
//! Mirror of `cocoa/leptos_cocoa/src/renderer_cocoa.rs`. CastFrom impls
//! live in `ios_dom::renderer` (orphan rule — see comment there).

#![allow(missing_docs)]

use ios_dom::{layout::Style, NodeKind, Renderer as IosRenderer};
use renderer::{
    renderer::Renderer as RendererTrait,
    view::Mountable,
};

pub use ios_dom::{
    ClassList, CssStyleDeclaration, Element, Event, Node, Placeholder,
    TemplateElement, Text,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Node = Node;
    type Element = Element;
    type Text = Text;
    type Placeholder = Placeholder;

    fn intern(text: &str) -> &str {
        IosRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> Text {
        IosRenderer::create_text_node(text)
    }

    fn create_placeholder() -> Placeholder {
        IosRenderer::create_placeholder()
    }

    fn set_text(node: &Text, text: &str) {
        IosRenderer::set_text(node, text);
    }

    fn set_attribute(node: &Element, name: &str, value: &str) {
        IosRenderer::set_attribute(node, name, value);
    }

    fn remove_attribute(node: &Element, name: &str) {
        IosRenderer::remove_attribute(node, name);
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
        let Some(parent_view) = before.ui_view().superview() else {
            return false;
        };
        let parent = synthesise_parent_element(parent_view, before);
        new_child.mount(&parent, Some(before));
        true
    }
}

impl Dom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no superview (must-succeed variant).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: &Node)
    where
        M: Mountable<Dom>,
    {
        let parent_view = before
            .ui_view()
            .superview()
            .expect("Dom::mount_before — node has no superview");
        let parent = synthesise_parent_element(parent_view, before);
        new_child.mount(&parent, Some(before));
    }
}

fn synthesise_parent_element(
    parent_view: ios_dom::Retained<ios_dom::UIView>,
    before: &Node,
) -> Element {
    use ios_dom::layout::LayoutHandle;

    let parent_handle: Option<LayoutHandle> = {
        let layout = before.layout_slot().borrow();
        layout.handle.as_ref().and_then(|h| {
            let parent_id = h.tree.tree.borrow().parent(h.node_id)?;
            Some(LayoutHandle {
                tree: h.tree.clone(),
                node_id: parent_id,
            })
        })
    };

    let parent_node = match parent_handle {
        Some(handle) => Node::from_view_with_handle(
            parent_view,
            NodeKind::Element,
            handle,
        ),
        None => Node::from_view(
            parent_view,
            NodeKind::Element,
            Style::default(),
        ),
    };
    Element::from_node_unchecked(parent_node)
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
        Vec::new()
    }
}

impl Mountable<Dom> for Element {
    fn unmount(&mut self) {
        self.as_node().teardown();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        <Dom as RendererTrait>::insert_node(parent, self.as_node(), marker);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<Element> {
        vec![self.clone()]
    }
}

impl Mountable<Dom> for Text {
    fn unmount(&mut self) {
        self.as_node().teardown();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        <Dom as RendererTrait>::insert_node(parent, self.as_node(), marker);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<Element> {
        Vec::new()
    }
}

impl Mountable<Dom> for Placeholder {
    fn unmount(&mut self) {
        self.as_node().teardown();
    }

    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        <Dom as RendererTrait>::insert_node(parent, self.as_node(), marker);
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<Element> {
        Vec::new()
    }
}
