//! The renderer surface that tachys targets.
//!
//! [`Renderer`] is a unit struct that mirrors the inherent-method surface
//! of `tachys::renderer::dom::Dom`: every method tachys ever calls on the
//! global renderer has a matching associated function here. This is the
//! "thin imperative API" that view types use to manipulate the tree.
//!
//! The methods that don't have a meaningful native counterpart (CSS
//! style declarations, class lists, `<template>` cloning, JS property
//! setting, hydration tree walking) are present so the type-checker is
//! happy, but they panic with `unimplemented!()` if actually called.

use crate::node::{Element, Node, Placeholder, Text};
use objc2_ui_kit::UIEvent;
use objc2::rc::Retained;
use send_wrapper::SendWrapper;
use std::fmt;

/// Marker / placeholder types that exist solely so tachys' generic
/// machinery has something concrete to alias. Most are never
/// constructed at runtime on the native target.
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

/// A UIKit event delivered to a handler. Currently a placeholder
/// wrapper around a `UIEvent`.
#[derive(Clone)]
pub struct Event {
    inner: Option<SendWrapper<Retained<UIEvent>>>,
}

impl Event {
    pub fn new(ev: Retained<UIEvent>) -> Self {
        Event {
            inner: Some(SendWrapper::new(ev)),
        }
    }

    pub fn synthetic() -> Self {
        Event { inner: None }
    }

    pub fn ui_event(&self) -> Option<&UIEvent> {
        self.inner.as_deref().map(|r| &**r)
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("has_ui_event", &self.inner.is_some())
            .finish()
    }
}

/// The renderer surface.
///
/// Aliased as `Dom` from inside tachys so that the rest of the codebase
/// (which calls `Rndr::create_element` and friends as `Dom::method`)
/// compiles without churn.
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
        parent.insert_node(new_child, anchor);
        true
    }

    pub fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        parent.remove_child(child)
    }

    pub fn remove(node: &Node) {
        node.ui_view().removeFromSuperview();
    }

    pub fn get_parent(_node: &Node) -> Option<Node> {
        unimplemented!(
            "ios_dom::Renderer::get_parent — hydration is not supported \
             on the native target"
        );
    }

    pub fn first_child(_node: &Node) -> Option<Node> {
        unimplemented!(
            "ios_dom::Renderer::first_child — hydration is not \
             supported on the native target"
        );
    }

    pub fn next_sibling(_node: &Node) -> Option<Node> {
        unimplemented!(
            "ios_dom::Renderer::next_sibling — hydration is not \
             supported on the native target"
        );
    }

    pub fn log_node(node: &Node) {
        eprintln!("[ios_dom] {node:?}");
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
    pub fn remove_css_property(
        _style: &CssStyleDeclaration,
        _name: &str,
    ) {
    }

    pub fn set_inner_html(_el: &Element, _html: &str) {
    }

    pub fn get_template<V: 'static>() -> TemplateElement {
        unimplemented!(
            "ios_dom::Renderer::get_template — <template> cloning is \
             a web-only optimization"
        );
    }

    pub fn clone_template(_tpl: &TemplateElement) -> Element {
        unimplemented!(
            "ios_dom::Renderer::clone_template — <template> cloning is \
             a web-only optimization"
        );
    }
}
