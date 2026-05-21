//! `NodeRef` — get a handle to a built element from outside its
//! `view!{}`. Mirrors `leptos_cocoa::cocoa::node_ref::NodeRef`.

use reactive_graph::{
    effect::Effect,
    signal::RwSignal,
    traits::{Get, GetUntracked, Set},
};
use std::cell::Cell;

// E is generally the element type (eg GtkElem)
#[derive(Debug)]
pub struct NodeRef<E>(RwSignal<Option<E>>);

impl<E: Copy + Send + Sync + 'static> NodeRef<E> {
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    pub fn get(&self) -> Option<E> {
        self.0.get()
    }

    pub fn get_untracked(&self) -> Option<E> {
        self.0.get_untracked()
    }

    pub fn on_load<F: FnOnce(E) + 'static>(self, f: F) {
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
    pub fn load(&self, el: E) {
        self.0.set(Some(el.clone()));
    }
}

impl<E: Copy + Send + Sync + 'static> Default for NodeRef<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> Clone for NodeRef<E> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<E> Copy for NodeRef<E> {}
