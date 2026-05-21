//! `NodeRef` — get a handle to a built element from outside its
//! `view!{}`. Mirrors `leptos_cocoa::cocoa::node_ref::NodeRef`.

use crate::dom::GtkElem;
use reactive_graph::{
    effect::Effect,
    signal::RwSignal,
    traits::{Get, GetUntracked, Set},
};
use std::cell::Cell;

#[derive(Debug)]
pub struct NodeRef(RwSignal<Option<GtkElem>>);

impl NodeRef {
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    pub fn get(&self) -> Option<GtkElem> {
        self.0.get()
    }

    pub fn get_untracked(&self) -> Option<GtkElem> {
        self.0.get_untracked()
    }

    pub fn on_load<F: FnOnce(GtkElem) + 'static>(self, f: F) {
        let f = Cell::new(Some(f));
        Effect::new(move |_| {
            if let Some(el) = self.get() {
                if let Some(f) = f.take() {
                    f(el);
                }
            }
        });
    }

    /// Internal: fill the ref. Called by builders' `Render::build`
    /// after constructing their underlying Node.
    pub fn load(&self, el: &GtkElem) {
        self.0.set(Some(el.clone()));
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
