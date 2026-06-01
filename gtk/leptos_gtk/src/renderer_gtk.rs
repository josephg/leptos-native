//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<GtkDom>`] / [`CastFrom`] impls that sit on the gtk_dom
//! types.
//!
//! Mirror of `leptos_cocoa::renderer_cocoa`. `Dom` is a unit struct
//! (not a type alias for `gtk_dom::Renderer`) so we can attach trait
//! impls and method extensions here without orphan-rule grief, and so
//! callers can write `<Dom as Renderer>::*` exactly as the
//! renderer-agnostic core (e.g. `common/renderer/src/view/iterators.rs`)
//! calls it.

#![allow(missing_docs)]

use crate::dom::layout;
use leptos_native::renderer::{
    Renderer,
    view::Mountable,
};

// Re-export the concrete tree types under the names the platform
// expects. `Text` and `Placeholder` are aliases for `Element` — the
// renderer trait wants distinct associated types, but on native
// they're all widget-backed Elements; the only thing distinguishing
// a "text node" or "placeholder" from a regular Element is the
// widget subclass + default style applied at creation.
pub use crate::dom::{
    Event, GtkElem, GtkMakeView, GtkNodeExt,
};
pub type Text = GtkElem;
pub type Placeholder = GtkElem;
use leptos_native::renderer::scene::LayoutBackend;

/// The GTK renderer surface — implements [`renderer::Renderer`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GtkDom;

impl Renderer for GtkDom {
    type Backend = layout::GtkBackend;
    type Node = GtkElem;

    fn intern(text: &str) -> &str {
        text
    }

    fn create_text_node(text: &str) -> GtkElem {
        GtkElem::create_text(text)
    }

    fn create_placeholder() -> GtkElem {
        GtkElem::create_placeholder()
    }

    fn set_text(node: GtkElem, text: &str) {
        node.set_text(text);
    }

    fn insert_node(
        parent: GtkElem,
        new_child: GtkElem,
        anchor: Option<GtkElem>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    fn remove_node(parent: GtkElem, child: GtkElem) -> Option<GtkElem> {
        parent.remove_child(child)
    }

    fn clear_children(parent: GtkElem) {
        parent.clear_children();
    }

    fn remove(node: GtkElem) {
        layout::drop_node(node);
    }

    fn get_parent(node: GtkElem) -> Option<GtkElem> {
        parent_of(node)
    }

    fn log_node(node: GtkElem) {
        eprintln!("[gtk_dom] {node:?}");
    }
}

impl GtkDom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no parent (mirror of `try_mount_before`).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: GtkElem)
    where
        M: Mountable<GtkDom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(parent, Some(before));
    }
}

/// The parent `Node` of `before` in the store, or `None` if it's a
/// root. The parent is a real node — no widget-wrapper synthesis is
/// needed under the thread-local store.
fn parent_of(before: GtkElem) -> Option<GtkElem> {
    crate::dom::layout::GtkBackend::parent(before.id())
        .map(GtkElem::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these live in this crate.
// ---------------------------------------------------------------------

impl Mountable<GtkDom> for GtkElem {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: GtkElem, marker: Option<GtkElem>) {
        <GtkDom as Renderer>::insert_node(parent, *self, marker);
    }

    fn try_mount(
        &mut self,
        parent: GtkElem,
        marker: Option<GtkElem>,
    ) -> bool {
        parent.insert_node(*self, marker)
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<GtkDom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<GtkElem> {
        vec![self.clone()]
    }
}

