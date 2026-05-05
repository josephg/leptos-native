//! `NodeRef` — get a handle to a built element from outside its
//! `view!{}`. Used for imperative operations that don't fit
//! reactive attributes / events: focus a text field, scroll a
//! container, query the underlying UIView for sizing.
//!
//! Mirrors `tachys::cocoa::node_ref::NodeRef`, monomorphic over
//! `ios_dom::Element` (no `<E>` parameter — there's only one element
//! type on iOS).

use ios_dom::Element;
use reactive_graph::{
    effect::Effect,
    signal::RwSignal,
    traits::{Get, GetUntracked, Set},
};
use send_wrapper::SendWrapper;
use std::cell::Cell;

/// A reactive reference to a built `ios_dom::Element`. Construct
/// via [`NodeRef::new`], pass to a builder via `node_ref=…`, then
/// read via [`get`](Self::get) / [`get_untracked`](Self::get_untracked) /
/// [`on_load`](Self::on_load).
#[derive(Debug)]
pub struct NodeRef(RwSignal<Option<SendWrapper<Element>>>);

impl NodeRef {
    /// Create a new, unfilled NodeRef. Filled when the builder
    /// it's attached to runs `Render::build`.
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    /// Reactive read — subscribes the current Effect to this ref.
    pub fn get(&self) -> Option<Element> {
        self.0.get().map(|w| w.take())
    }

    /// Non-reactive read.
    pub fn get_untracked(&self) -> Option<Element> {
        self.0.get_untracked().map(|w| w.take())
    }

    /// Run `f` once when the ref is filled. Useful for "focus this
    /// field on first show" patterns.
    pub fn on_load<F: FnOnce(Element) + 'static>(self, f: F) {
        let f = Cell::new(Some(f));
        Effect::new(move |_| {
            if let Some(el) = self.get() {
                if let Some(f) = f.take() {
                    f(el);
                }
            }
        });
    }

    /// Internal: fill the ref. Called by builders' `Render::build`.
    pub fn load(&self, el: &Element) {
        self.0.set(Some(SendWrapper::new(el.clone())));
    }
}

impl Default for NodeRef {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for NodeRef {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Copy for NodeRef {}
