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
//! On the native code path they should be unreachable; tracked in
//! `implementation_log.md`.

use crate::node::{Element, CocoaNode};
use objc2_app_kit::NSEvent;
use objc2::rc::Retained;
use send_wrapper::SendWrapper;
use std::fmt;

/// Marker / placeholder types that exist solely so tachys' generic
/// machinery has something concrete to alias. Most are never
/// constructed at runtime on the native target.
///
/// Stage 3 will give [`Event`] a real implementation (carrying an
/// `NSEvent`); Stage 5 may revisit the others as the styling story for
/// AppKit firms up.
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

/// An AppKit event delivered to a handler. Currently a placeholder
/// wrapper around an `NSEvent`; will be fleshed out in Stage 3 alongside
/// the event-dispatch wiring.
#[derive(Clone)]
pub struct Event {
    inner: Option<SendWrapper<Retained<NSEvent>>>,
}

impl Event {
    pub fn new(ev: Retained<NSEvent>) -> Self {
        Event {
            inner: Some(SendWrapper::new(ev)),
        }
    }

    /// A synthetic event with no payload — used for synthesized
    /// notifications (e.g. button target/action that doesn't carry a
    /// real NSEvent).
    pub fn synthetic() -> Self {
        Event { inner: None }
    }

    pub fn ns_event(&self) -> Option<&NSEvent> {
        self.inner.as_deref().map(|r| &**r)
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("has_ns_event", &self.inner.is_some())
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
        // Web's wasm-bindgen string interning has no native equivalent.
        text
    }

    pub fn create_text_node(text: &str) -> Element {
        Element::create_text(text)
    }

    pub fn create_placeholder() -> Element {
        Element::create_placeholder()
    }

    pub fn set_text(node: &Element, text: &str) {
        node.set_text(text);
    }

    pub fn insert_node(
        parent: &Element,
        new_child: &CocoaNode,
        anchor: Option<&CocoaNode>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    pub fn try_insert_node(
        parent: &Element,
        new_child: &CocoaNode,
        anchor: Option<&CocoaNode>,
    ) -> bool {
        parent.insert_node(new_child, anchor);
        true
    }

    pub fn remove_node(parent: &Element, child: &CocoaNode) -> Option<CocoaNode> {
        parent.remove_child(child)
    }

    pub fn remove(node: &CocoaNode) {
        // `drop_node` detaches the NSView and removes the node (and its
        // structural subtree) from the store, so the node count returns
        // to baseline. Caught by `cocoa_fuzzer`'s post-seed check.
        crate::layout::drop_node(*node);
    }

    pub fn log_node(node: &CocoaNode) {
        eprintln!("[cocoa_dom] {node:?}");
    }

    pub fn clear_children(parent: &Element) {
        parent.clear_children();
    }
}

// ---------------------------------------------------------------------
// CastFrom impls — used by leptos_cocoa::Dom and the renderer-agnostic
// view tree. They live here (not in leptos_cocoa) because of the orphan
// rule: CastFrom is a foreign trait, Node/Element are local to this
// crate, and the trait doesn't mention any local type.
// ---------------------------------------------------------------------

use renderer::renderer::CastFrom;

impl CastFrom<CocoaNode> for CocoaNode {
    fn cast_from(source: CocoaNode) -> Option<CocoaNode> {
        Some(source)
    }
}
