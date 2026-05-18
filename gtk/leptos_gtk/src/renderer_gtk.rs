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

use gtk4::prelude::*;
use gtk_dom::{
    layout::Style, NodeKind, Renderer as GtkRenderer,
};
use renderer::{
    renderer::Renderer as RendererTrait,
    view::Mountable,
};

// Re-export the concrete tree types under the names the platform
// expects.
pub use gtk_dom::{
    ClassList, CssStyleDeclaration, Element, Event, Node, Placeholder,
    TemplateElement, Text,
};

/// The GTK renderer surface — implements [`renderer::Renderer`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Node = Node;
    type Element = Element;
    type Text = Text;
    type Placeholder = Placeholder;

    fn intern(text: &str) -> &str {
        GtkRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> Text {
        GtkRenderer::create_text_node(text)
    }

    fn create_placeholder() -> Placeholder {
        GtkRenderer::create_placeholder()
    }

    fn set_text(node: &Text, text: &str) {
        GtkRenderer::set_text(node, text);
    }

    fn set_attribute(node: &Element, name: &str, value: &str) {
        GtkRenderer::set_attribute(node, name, value);
    }

    fn remove_attribute(node: &Element, name: &str) {
        GtkRenderer::remove_attribute(node, name);
    }

    fn insert_node(
        parent: &Element,
        new_child: &Node,
        anchor: Option<&Node>,
    ) {
        GtkRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        GtkRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: &Element) {
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
        let Some(parent_widget) = before.widget().parent() else {
            return false;
        };
        let parent = synthesise_parent_element(parent_widget, before);
        new_child.mount(&parent, Some(before));
        true
    }
}

impl Dom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no parent (mirror of `try_mount_before` for
    /// callers that know there's a parent).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: &Node)
    where
        M: Mountable<Dom>,
    {
        let parent_widget = before
            .widget()
            .parent()
            .expect("Dom::mount_before — node has no parent");
        let parent = synthesise_parent_element(parent_widget, before);
        new_child.mount(&parent, Some(before));
    }
}

/// Build an `Element` wrapper around `parent_widget` whose
/// `LayoutHandle` references the same Taffy tree + the parent
/// `NodeId` that `before` lives under. If `before` isn't registered
/// in any tree, the parent wrapper also has no handle — falls back
/// to GTK-only mounting.
fn synthesise_parent_element(
    parent_widget: gtk4::Widget,
    before: &Node,
) -> Element {
    use gtk_dom::layout::LayoutHandle;

    let parent_handle: Option<LayoutHandle> = before
        .mounted_handle()
        .and_then(|h| {
            let parent_id = h.tree.parent(h.node_id)?;
            Some(LayoutHandle {
                tree: h.tree.clone(),
                node_id: parent_id,
            })
        });

    let parent_node = match parent_handle {
        Some(handle) => Node::from_widget_with_handle(
            parent_widget,
            NodeKind::Element,
            handle,
        ),
        None => Node::from_widget(
            parent_widget,
            NodeKind::Element,
            Style::default(),
        ),
    };
    Element::from_node_unchecked(parent_node)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these live in this crate.
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
        GtkRenderer::try_insert_node(parent, self, marker)
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
