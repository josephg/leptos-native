//! `NodeRef` — get a handle to a built element from outside its
//! `view!{}`. Used for imperative operations that don't fit
//! reactive attributes / events: focus a text field, scroll a
//! container, query the underlying NSView for sizing.
//!
//! Mirrors the upstream `leptos_native::NodeRef<HtmlInputCocoaNode>` API
//! shape but is monomorphic — there's only one element type on
//! macOS (`cocoa_dom::CocoaNode`), so no `<E>` parameter is
//! needed.
//!
//! # Usage
//!
//! ```ignore
//! let username = NodeRef::new();
//! view! {
//!     <text_field node_ref=username placeholder="Name" />
//!     <button on:click=move |_| {
//!         if let Some(el) = username.get() {
//!             // imperative: focus the field
//!             el.focus();
//!         }
//!     }>"Focus name"</button>
//! }
//! ```

use reactive_graph::{
    effect::Effect,
    signal::RwSignal,
    traits::{Get, GetUntracked, Set},
};
use std::cell::Cell;
use crate::dom::CocoaElem;

/// A reactive reference to a built `cocoa_dom::CocoaNode`.
///
/// Construct via [`NodeRef::new`], pass to a builder via
/// `node_ref=…`, then read via [`get`](Self::get) /
/// [`get_untracked`](Self::get_untracked) / [`on_load`](Self::on_load).
///
/// The wrapped signal is `RwSignal<Option<SendWrapper<CocoaNode>>>`.
/// The `SendWrapper` keeps the type `Send` (required by
/// reactive_graph's storage) while runtime-enforcing main-thread
/// access.
#[derive(Debug)]
pub struct NodeRef(RwSignal<Option<CocoaElem>>);

impl NodeRef {
    /// Create a new, unfilled NodeRef. Filled when the builder
    /// it's attached to runs `Render::build`.
    #[track_caller]
    pub fn new() -> Self {
        Self(RwSignal::new(None))
    }

    /// Reactive read — subscribes the current Effect to this
    /// ref. Returns the element if it's been mounted, else None.
    pub fn get(&self) -> Option<CocoaElem> {
        self.0.get()
    }

    /// Non-reactive read.
    pub fn get_untracked(&self) -> Option<CocoaElem> {
        self.0.get_untracked()
    }

    /// Run `f` once when the ref has been filled (i.e. when the
    /// element it points to has been built and mounted). Useful
    /// for "focus this field on first show" patterns.
    pub fn on_load<F: FnOnce(CocoaElem) + 'static>(self, f: F) {
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
    /// after constructing their underlying CocoaNode.
    pub fn load(&self, el: CocoaElem) {
        self.0.set(Some(el));
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

// `RwSignal<T>` is Copy so this is too.
impl Copy for NodeRef {}
