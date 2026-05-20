//! `NodeRef` — get a handle to a built element from outside its
//! `view!{}`. Mirrors `leptos_cocoa::cocoa::node_ref::NodeRef`.

use gtk_dom::Node;
use reactive_graph::{
    effect::Effect,
    signal::RwSignal,
    traits::{Get, GetUntracked, Set},
};
use send_wrapper::SendWrapper;
use std::cell::Cell;

#[derive(Debug)]
pub struct NodeRef(RwSignal<Option<SendWrapper<Node>>>);

impl NodeRef {
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    pub fn get(&self) -> Option<Node> {
        self.0.get().map(|w| w.take())
    }

    pub fn get_untracked(&self) -> Option<Node> {
        self.0.get_untracked().map(|w| w.take())
    }

    pub fn on_load<F: FnOnce(Node) + 'static>(self, f: F) {
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
    pub fn load(&self, el: &Node) {
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
