//! The renderer surface that leptos_gtk targets.
//!
//! [`Renderer`] is a unit struct mirroring the inherent-method surface
//! of `tachys::renderer::dom::Dom`: every method tachys ever calls on
//! the global renderer has a matching associated function here.
//!
//! Methods without a meaningful native counterpart (CSS style
//! declarations, class lists, `<template>` cloning, JS property
//! setting, hydration tree walking) panic with `unimplemented!()` if
//! actually called.

use crate::node::{Element, Node, NodeKind, Placeholder, Text};
use send_wrapper::SendWrapper;
use std::fmt;

/// Marker / placeholder types that exist solely so tachys' generic
/// machinery has something concrete to alias.
#[derive(Clone, Default)]
pub struct ClassList;

impl fmt::Debug for ClassList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClassList(<unsupported on native>)")
    }
}

#[derive(Clone, Default)]
pub struct CssStyleDeclaration;

impl fmt::Debug for CssStyleDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CssStyleDeclaration(<unsupported on native>)")
    }
}

#[derive(Clone, Default)]
pub struct TemplateElement;

impl fmt::Debug for TemplateElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TemplateElement(<unsupported on native>)")
    }
}

/// A GTK event delivered to a handler. Currently a placeholder
/// wrapper around an optional `gdk::Event`.
#[derive(Clone)]
pub struct Event {
    inner: Option<SendWrapper<gtk4::gdk::Event>>,
}

impl Event {
    pub fn new(ev: gtk4::gdk::Event) -> Self {
        Event {
            inner: Some(SendWrapper::new(ev)),
        }
    }

    pub fn synthetic() -> Self {
        Event { inner: None }
    }

    pub fn gdk_event(&self) -> Option<&gtk4::gdk::Event> {
        self.inner.as_deref()
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("has_gdk_event", &self.inner.is_some())
            .finish()
    }
}

/// The renderer surface.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Renderer;

impl Renderer {
    pub fn intern(text: &str) -> &str {
        text
    }

    pub fn create_element(tag: &str, _namespace: Option<&str>) -> Element {
        Element::create(tag)
    }

    pub fn create_text_node(text: &str) -> Text {
        Text::create(text)
    }

    pub fn create_placeholder() -> Placeholder {
        Placeholder::create()
    }

    pub fn set_text(node: &Text, text: &str) {
        node.set_text(text);
    }

    pub fn set_attribute(node: &Element, name: &str, value: &str) {
        node.set_attribute(name, value);
    }

    pub fn remove_attribute(node: &Element, name: &str) {
        node.remove_attribute(name);
    }

    pub fn insert_node(
        parent: &Element,
        new_child: &Node,
        anchor: Option<&Node>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    pub fn try_insert_node(
        parent: &Element,
        new_child: &Node,
        anchor: Option<&Node>,
    ) -> bool {
        parent.try_insert_node(new_child, anchor)
    }

    pub fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        parent.remove_child(child)
    }

    pub fn remove(node: &Node) {
        node.teardown();
    }

    pub fn get_parent(_node: &Node) -> Option<Node> {
        unimplemented!(
            "gtk_dom::Renderer::get_parent — hydration is not supported \
             on the native target"
        );
    }

    pub fn first_child(_node: &Node) -> Option<Node> {
        unimplemented!(
            "gtk_dom::Renderer::first_child — hydration is not supported \
             on the native target"
        );
    }

    pub fn next_sibling(_node: &Node) -> Option<Node> {
        unimplemented!(
            "gtk_dom::Renderer::next_sibling — hydration is not supported \
             on the native target"
        );
    }

    pub fn log_node(node: &Node) {
        eprintln!("[gtk_dom] {node:?}");
    }

    pub fn clear_children(parent: &Element) {
        parent.clear_children();
    }

    // ---- DOM-only / web-only stubs --------------------------------

    pub fn class_list(_el: &Element) -> ClassList {
        ClassList
    }
    pub fn add_class(_list: &ClassList, _name: &str) {}
    pub fn remove_class(_list: &ClassList, _name: &str) {}

    pub fn style(_el: &Element) -> CssStyleDeclaration {
        CssStyleDeclaration
    }
    pub fn set_css_property(
        _style: &CssStyleDeclaration,
        _name: &str,
        _value: &str,
    ) {
    }
    pub fn remove_css_property(_style: &CssStyleDeclaration, _name: &str) {}

    pub fn set_inner_html(_el: &Element, _html: &str) {}

    pub fn get_template<V: 'static>() -> TemplateElement {
        unimplemented!(
            "gtk_dom::Renderer::get_template — <template> cloning is a \
             web-only optimization"
        );
    }

    pub fn clone_template(_tpl: &TemplateElement) -> Element {
        unimplemented!(
            "gtk_dom::Renderer::clone_template — <template> cloning is a \
             web-only optimization"
        );
    }
}

// ---------------------------------------------------------------------
// CastFrom impls — used by leptos_gtk::Dom and the renderer-agnostic
// view tree. They live here for the same orphan-rule reasons as the
// cocoa_dom CastFrom impls do.
// ---------------------------------------------------------------------

use renderer::renderer::CastFrom;

impl CastFrom<Node> for Element {
    fn cast_from(node: Node) -> Option<Element> {
        match node.kind() {
            NodeKind::Element => Some(Element::from_node_unchecked(node)),
            _ => None,
        }
    }
}

impl CastFrom<Node> for Text {
    fn cast_from(node: Node) -> Option<Text> {
        match node.kind() {
            NodeKind::Text => Some(Text::from_node_unchecked(node)),
            _ => None,
        }
    }
}

impl CastFrom<Node> for Placeholder {
    fn cast_from(node: Node) -> Option<Placeholder> {
        match node.kind() {
            NodeKind::Placeholder => {
                Some(Placeholder::from_node_unchecked(node))
            }
            _ => None,
        }
    }
}

impl CastFrom<Element> for Element {
    fn cast_from(source: Element) -> Option<Element> {
        Some(source)
    }
}
