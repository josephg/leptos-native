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

use crate::dom::node::GtkElem;
use send_wrapper::SendWrapper;
use std::fmt;

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
