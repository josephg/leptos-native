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

use objc2_ui_kit::UIEvent;
use objc2::rc::Retained;
use send_wrapper::SendWrapper;
use std::fmt;

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
