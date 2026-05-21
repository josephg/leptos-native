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

use crate::dom::node::GtkNode;
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

    pub fn create_text_node(text: &str) -> GtkNode {
        GtkNode::create_text(text)
    }

    pub fn create_placeholder() -> GtkNode {
        GtkNode::create_placeholder()
    }

    pub fn set_text(node: &GtkNode, text: &str) {
        node.set_text(text);
    }

    pub fn insert_node(
        parent: &GtkNode,
        new_child: &GtkNode,
        anchor: Option<&GtkNode>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    pub fn try_insert_node(
        parent: &GtkNode,
        new_child: &GtkNode,
        anchor: Option<&GtkNode>,
    ) -> bool {
        parent.try_insert_node(new_child, anchor)
    }

    pub fn remove_node(parent: &GtkNode, child: &GtkNode) -> Option<GtkNode> {
        parent.remove_child(child)
    }

    pub fn remove(node: &GtkNode) {
        layout::drop_node(node);
    }

    pub fn get_parent(_node: &GtkNode) -> Option<GtkNode> {
        unimplemented!(
            "gtk_dom::Renderer::get_parent — hydration is not supported \
             on the native target"
        );
    }

    pub fn first_child(_node: &GtkNode) -> Option<GtkNode> {
        unimplemented!(
            "gtk_dom::Renderer::first_child — hydration is not supported \
             on the native target"
        );
    }

    pub fn next_sibling(_node: &GtkNode) -> Option<GtkNode> {
        unimplemented!(
            "gtk_dom::Renderer::next_sibling — hydration is not supported \
             on the native target"
        );
    }

    pub fn log_node(node: &GtkNode) {
        eprintln!("[gtk_dom] {node:?}");
    }

    pub fn clear_children(parent: &GtkNode) {
        parent.clear_children();
    }

    // ---- DOM-only / web-only stubs --------------------------------

    pub fn class_list(_el: &GtkNode) -> ClassList {
        ClassList
    }
    pub fn add_class(_list: &ClassList, _name: &str) {}
    pub fn remove_class(_list: &ClassList, _name: &str) {}

    pub fn style(_el: &GtkNode) -> CssStyleDeclaration {
        CssStyleDeclaration
    }
    pub fn set_css_property(
        _style: &CssStyleDeclaration,
        _name: &str,
        _value: &str,
    ) {
    }
    pub fn remove_css_property(_style: &CssStyleDeclaration, _name: &str) {}

    pub fn set_inner_html(_el: &GtkNode, _html: &str) {}

    pub fn get_template<V: 'static>() -> TemplateElement {
        unimplemented!(
            "gtk_dom::Renderer::get_template — <template> cloning is a \
             web-only optimization"
        );
    }

    pub fn clone_template(_tpl: &TemplateElement) -> GtkNode {
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
use crate::dom::layout;

impl CastFrom<GtkNode> for GtkNode {
    fn cast_from(source: GtkNode) -> Option<GtkNode> {
        Some(source)
    }
}
