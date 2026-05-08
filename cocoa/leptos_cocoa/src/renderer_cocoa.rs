//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] / [`CastFrom`] impls that sit on the cocoa_dom
//! types.
//!
//! `Dom` is a unit struct (not a type alias for `cocoa_dom::Renderer`)
//! so we can attach trait impls and method extensions here without
//! orphan-rule grief, and so callers can write `<Dom as Renderer>::*`
//! exactly as the renderer-agnostic core (e.g. `common/renderer/src/
//! view/iterators.rs`) calls it.

#![allow(missing_docs)]

use cocoa_dom::{
    layout::Style, NodeKind, Renderer as CocoaRenderer,
};
use renderer::{
    renderer::Renderer as RendererTrait,
    view::Mountable,
};

// Re-export the concrete tree types under the names tachys/leptos/
// the platform expects.
pub use cocoa_dom::{
    ClassList, CssStyleDeclaration, Element, Event, Node, Placeholder,
    TemplateElement, Text,
};

/// The Cocoa renderer surface — implements [`renderer::Renderer`].
///
/// All methods forward to [`cocoa_dom::Renderer`]; this type exists
/// so we can attach the trait impl + the additional `synthesise_parent_*`
/// helpers below.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Node = Node;
    type Element = Element;
    type Text = Text;
    type Placeholder = Placeholder;

    fn intern(text: &str) -> &str {
        CocoaRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> Text {
        CocoaRenderer::create_text_node(text)
    }

    fn create_placeholder() -> Placeholder {
        CocoaRenderer::create_placeholder()
    }

    fn set_text(node: &Text, text: &str) {
        CocoaRenderer::set_text(node, text);
    }

    fn set_attribute(node: &Element, name: &str, value: &str) {
        CocoaRenderer::set_attribute(node, name, value);
    }

    fn remove_attribute(node: &Element, name: &str) {
        CocoaRenderer::remove_attribute(node, name);
    }

    fn insert_node(
        parent: &Element,
        new_child: &Node,
        anchor: Option<&Node>,
    ) {
        CocoaRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        CocoaRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: &Element) {
        CocoaRenderer::clear_children(parent);
    }

    fn remove(node: &Node) {
        CocoaRenderer::remove(node);
    }

    fn get_parent(node: &Node) -> Option<Node> {
        // The default `try_mount_before` impl on the trait calls
        // get_parent. cocoa_dom's get_parent panics with a hydration
        // message; here we return None so try_mount_before falls back.
        // Real native callers (For/Vec) go through synthesise_parent_*
        // helpers below, not this method.
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
        CocoaRenderer::log_node(node);
    }

    /// Override the trait's default — on cocoa we need to synthesise a
    /// parent Element wrapper around `before`'s superview, with the
    /// right LayoutHandle so the new child registers in the same Taffy
    /// tree.
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: &Node) -> bool
    where
        M: Mountable<Self>,
    {
        let superview = unsafe { before.ns_view().superview() };
        let Some(parent_view) = superview else {
            return false;
        };
        let parent = synthesise_parent_element(parent_view, before);
        new_child.mount(&parent, Some(before));
        true
    }
}

impl Dom {
    /// Mount `new_child` immediately before `before`. Panics if `before`
    /// has no superview (mirror of `try_mount_before` for callers that
    /// know there's a parent).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: &Node)
    where
        M: Mountable<Dom>,
    {
        let parent_view = unsafe { before.ns_view().superview() }
            .expect("Dom::mount_before — node has no superview");
        let parent = synthesise_parent_element(parent_view, before);
        new_child.mount(&parent, Some(before));
    }
}

/// Build an `Element` wrapper around `parent_view` whose `LayoutHandle`
/// references the same Taffy tree + the parent `NodeId` that `before`
/// lives under. If `before` isn't registered in any tree, the parent
/// wrapper also has no handle — falls back to NSView-only mounting.
fn synthesise_parent_element(
    parent_view: cocoa_dom::Retained<cocoa_dom::NSView>,
    before: &Node,
) -> Element {
    use cocoa_dom::layout::LayoutHandle;

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
// Mountable<Dom> impls — orphan-rule says these have to live in this
// crate (Dom is local; Node/Element/etc. are local re-exports? Actually
// they're from cocoa_dom — but Mountable<Dom> picks up via the local
// `Dom` parameter, so the impls are valid here).
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
        CocoaRenderer::try_insert_node(parent, self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        // No good way back from a raw NSView to a typed parent Element
        // without the Taffy bookkeeping `synthesise_parent_element` does.
        // Callers that need this go through Dom::try_mount_before.
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

// CastFrom impls live in cocoa_dom (orphan rule: both the trait and
// the types must satisfy locality, and CastFrom doesn't mention Dom,
// so the impls have to sit with Node/Element/Text/Placeholder).
