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

use super::node::UikitElem;
use objc2_ui_kit::UIEvent;
use objc2::rc::Retained;
use send_wrapper::SendWrapper;
use std::fmt;
use leptos_native::renderer::CastFrom;
use crate::dom::layout;

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

    pub fn create_text_node(text: &str) -> UikitElem {
        UikitElem::create_text(text)
    }

    pub fn create_placeholder() -> UikitElem {
        UikitElem::create_placeholder()
    }

    pub fn set_text(node: UikitElem, text: &str) {
        node.set_text(text);
    }

    pub fn insert_node(
        parent: UikitElem,
        new_child: UikitElem,
        anchor: Option<UikitElem>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    pub fn try_insert_node(
        parent: UikitElem,
        new_child: UikitElem,
        anchor: Option<UikitElem>,
    ) -> bool {
        parent.insert_node(new_child, anchor);
        true
    }

    pub fn remove_node(parent: UikitElem, child: UikitElem) -> Option<UikitElem> {
        parent.remove_child(child)
    }

    pub fn remove(node: UikitElem) {
        // Detach the view and remove the node (and its structural
        // subtree) from the store, returning node count to baseline.
        layout::drop_node(node);
    }

    pub fn get_parent(_node: UikitElem) -> Option<UikitElem> {
        unimplemented!(
            "ios_dom::Renderer::get_parent — hydration is not supported \
             on the native target"
        );
    }

    pub fn log_node(node: UikitElem) {
        eprintln!("[ios_dom] {node:?}");
    }

    pub fn clear_children(parent: UikitElem) {
        parent.clear_children();
    }
}


// ---------------------------------------------------------------------
// CastFrom impls (orphan rule — Node/Element are local to ios_dom;
// CastFrom is from `renderer`, which has no local-type reference, so
// the impls have to live here).
// ---------------------------------------------------------------------

impl CastFrom<UikitElem> for UikitElem {
    fn cast_from(source: UikitElem) -> Option<UikitElem> {
        Some(source)
    }
}
