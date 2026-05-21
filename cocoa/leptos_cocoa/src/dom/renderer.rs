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

use objc2::rc::Retained;
use objc2_app_kit::NSEvent;
use send_wrapper::SendWrapper;
use std::fmt;

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

// ---------------------------------------------------------------------
// CastFrom impls — used by leptos_cocoa::Dom and the renderer-agnostic
// view tree. They live here (not in leptos_cocoa) because of the orphan
// rule: CastFrom is a foreign trait, Node/Element are local to this
// crate, and the trait doesn't mention any local type.
// ---------------------------------------------------------------------

// impl CastFrom<CocoaElem> for CocoaElem {
//     fn cast_from(source: CocoaElem) -> Option<CocoaElem> {
//         Some(source)
//     }
// }
