//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] impls on the ios_dom types.
//!
//! Mirror of `cocoa/leptos_cocoa/src/renderer_cocoa.rs`. CastFrom impls
//! live in `ios_dom::renderer` (orphan rule — see comment there).

#![allow(missing_docs)]

use crate::dom::layout::IosBackend;
use crate::dom::Renderer as IosRenderer;
use leptos_native::renderer::LayoutBackend;
use leptos_native::renderer::{
    view::Mountable,
    Renderer as RendererTrait,
};

// `Text` and `Placeholder` are aliases for `UikitElem` — the renderer
// trait wants distinct associated types, but on native they're all
// UIView-backed UikitElems; the only thing distinguishing a "text node"
// or "placeholder" from a regular UikitElem is the UIView subclass +
// default style applied at creation.
pub use crate::dom::{
    Event, UikitElem,
};
pub type Text = UikitElem;
pub type Placeholder = UikitElem;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Backend = IosBackend;
    type Node = UikitElem;

    fn intern(text: &str) -> &str {
        IosRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> UikitElem {
        IosRenderer::create_text_node(text)
    }

    fn create_placeholder() -> UikitElem {
        IosRenderer::create_placeholder()
    }

    fn set_text(node: UikitElem, text: &str) {
        IosRenderer::set_text(node, text);
    }

    fn insert_node(
        parent: UikitElem,
        new_child: UikitElem,
        anchor: Option<UikitElem>,
    ) {
        IosRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: UikitElem, child: UikitElem) -> Option<UikitElem> {
        IosRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: UikitElem) {
        IosRenderer::clear_children(parent);
    }

    fn remove(node: UikitElem) {
        IosRenderer::remove(node);
    }

    fn get_parent(node: UikitElem) -> Option<UikitElem> {
        // The parent is a real node in the store; look it up by id.
        // Used by `UnitState::insert_before_this` (the mount anchor
        // for `<Switch>` and other placeholder-based control-flow).
        IosBackend::parent(node.id())
            .map(UikitElem::from_id)
    }

    fn log_node(node: UikitElem) {
        IosRenderer::log_node(node);
    }

    /// Override the trait default: synthesise a parent UikitElem from
    /// `before`'s superview, with the right LayoutHandle so the new
    /// child registers in the same Taffy tree.
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: UikitElem) -> bool
    where
        M: Mountable<Self>,
    {
        let Some(parent) = parent_of(before) else {
            return false;
        };
        new_child.mount(parent, Some(before));
        true
    }
}

impl Dom {
    /// Mount `new_child` immediately before `before`. Panics if
    /// `before` has no parent (must-succeed variant).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: UikitElem)
    where
        M: Mountable<Dom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(parent, Some(before));
    }
}

/// The parent `UikitElem` of `before` in the store, or `None` if it's a
/// root. No view-wrapper synthesis needed under the thread-local store.
fn parent_of(before: UikitElem) -> Option<UikitElem> {
    IosBackend::parent(before.id())
        .map(UikitElem::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls
// ---------------------------------------------------------------------

/// Body for [`Mountable::insert_before_this`]: mount `child` before
/// `before` in `before`'s parent. Returns `false` if `before` has no
/// parent (detached or a scene root), so callers fall back to another
/// anchor.
pub(crate) fn insert_before_node(
    before: UikitElem,
    child: &mut dyn Mountable<Dom>,
) -> bool {
    let Some(parent) = parent_of(before) else {
        return false;
    };
    child.mount(parent, Some(before));
    true
}

impl Mountable<Dom> for UikitElem {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: UikitElem, marker: Option<UikitElem>) {
        <Dom as RendererTrait>::insert_node(parent, *self, marker);
    }

    fn try_mount(
        &mut self,
        parent: UikitElem,
        marker: Option<UikitElem>,
    ) -> bool {
        IosRenderer::try_insert_node(parent, *self, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<Dom>) -> bool {
        insert_before_node(*self, child)
    }

    fn elements(&self) -> Vec<UikitElem> {
        vec![self.clone()]
    }
}

