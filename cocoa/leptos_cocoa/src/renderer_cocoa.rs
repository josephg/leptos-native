//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<CocoaDom>`] / [`CastFrom`] impls that sit on the cocoa_dom
//! types.
//!
//! `Dom` is a unit struct (not a type alias for `cocoa_dom::Renderer`)
//! so we can attach trait impls and method extensions here without
//! orphan-rule grief, and so callers can write `<Dom as Renderer>::*`
//! exactly as the renderer-agnostic core (e.g. `common/renderer/src/
//! view/iterators.rs`) calls it.

#![allow(missing_docs)]

use crate::dom::layout::CocoaBackend;
use leptos_native::renderer::{Renderer, view::Mountable, LayoutBackend};

// Re-export the concrete tree types under the names tachys/leptos/
// the platform expects. `Text` and `Placeholder` are aliases for
// `Element` — the renderer trait wants distinct associated types,
// but on native they're all NSView-backed Elements; the only thing
// distinguishing a "text node" or "placeholder" from a regular
// Element is the NSView subclass + default style applied at creation.
pub use crate::dom::{
    CocoaElem, Event,
};
use crate::dom::CocoaNodeExt;
use crate::dom::layout;

pub type Text = CocoaElem;
pub type Placeholder = CocoaElem;

/// The Cocoa renderer surface — implements [`renderer::Renderer`].
///
/// All methods forward to [`cocoa_dom::Renderer`]; this type exists
/// so we can attach the trait impl + the additional `synthesise_parent_*`
/// helpers below.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CocoaDom;

impl Renderer for CocoaDom {
    type Backend = CocoaBackend;
    type Node = CocoaElem;

    fn create_text_node(text: &str) -> CocoaElem {
        CocoaElem::create_text(text)
    }

    fn create_placeholder() -> CocoaElem {
        CocoaElem::create_placeholder()
    }

    fn set_text(node: CocoaElem, text: &str) {
        node.set_text(text);
    }

    fn insert_node(
        parent: CocoaElem,
        new_child: CocoaElem,
        anchor: Option<CocoaElem>,
    ) {
        parent.insert_node(new_child, anchor);
    }

    fn remove_node(parent: CocoaElem, child: CocoaElem) -> Option<CocoaElem> {
        parent.remove_child(child)
    }

    fn clear_children(parent: CocoaElem) {
        parent.clear_children();
    }

    fn remove(node: CocoaElem) {
        layout::drop_node(node);
    }

    fn get_parent(node: CocoaElem) -> Option<CocoaElem> {
        // The parent is a real node in the store; look it up by id.
        // Used by `UnitState::insert_before_this` (the mount anchor
        // for `<Switch>` and other placeholder-based control-flow).
        CocoaBackend::parent(node.id())
            .map(CocoaElem::from_id)
    }

    fn log_node(node: CocoaElem) {
        eprintln!("[cocoa_dom] {node:?}");
    }
}

impl CocoaDom {
    /// Mount `new_child` immediately before `before`. Panics if `before`
    /// has no parent (mirror of `try_mount_before`).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: CocoaElem)
    where
        M: Mountable<CocoaDom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(parent, Some(before));
    }
}

/// The parent `Node` of `before` in the store, or `None` if `before`
/// is a root (or detached). The parent is a real node; no NSView
/// wrapper synthesis is needed under the thread-local store.
pub(crate) fn parent_of(before: CocoaElem) -> Option<CocoaElem> {
    CocoaBackend::parent(before.id())
        .map(CocoaElem::from_id)
}

// ---------------------------------------------------------------------
// Mountable<Dom> impls — orphan-rule says these have to live in this
// crate (Dom is local; Node/Element/etc. are local re-exports? Actually
// they're from cocoa_dom — but Mountable<Dom> picks up via the local
// `Dom` parameter, so the impls are valid here).
// ---------------------------------------------------------------------

/// Shared body for [`Mountable::insert_before_this`] across `Node`
/// and `Element`. Walks the NSView's superview, synthesises a parent
/// `Element` wrapper, mounts `child` before `before` in that parent.
///
/// Returns `false` if `before` has no superview (it's detached or is
/// the window's root content view). Callers fall back to mounting at
/// a different anchor in that case.
pub(crate) fn insert_before_node(
    before: CocoaElem,
    child: &mut dyn Mountable<CocoaDom>,
) -> bool {
    let Some(parent) = parent_of(before) else {
        return false;
    };
    child.mount(parent, Some(before));
    true
}

impl Mountable<CocoaDom> for CocoaElem {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: CocoaElem, marker: Option<CocoaElem>) {
        CocoaDom::insert_node(parent, *self, marker);
    }

    fn try_mount(
        &mut self,
        parent: CocoaElem,
        marker: Option<CocoaElem>,
    ) -> bool {
        // TODO: This should be actually fallable.
        parent.insert_node(*self, marker);
        true
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<CocoaDom>) -> bool {
        insert_before_node(*self, child)
    }

    fn elements(&self) -> Vec<CocoaElem> {
        vec![self.clone()]
    }
}

// CastFrom impls live in cocoa_dom (orphan rule: both the trait and
// the types must satisfy locality, and CastFrom doesn't mention Dom,
// so the impls have to sit with Node/Element/Text/Placeholder).
