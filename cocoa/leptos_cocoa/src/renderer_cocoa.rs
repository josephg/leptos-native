//! `Dom`: this crate's [`renderer::Renderer`] impl, plus the orphan-rule
//! [`Mountable<Dom>`] / [`CastFrom`] impls that sit on the cocoa_dom
//! types.
//!
//! `Dom` is a unit struct (not a type alias for `cocoa_dom::Renderer`)
//! so we can attach trait impls and method extensions here without
//! orphan-rule grief, and so callers can write `<Dom as Renderer>::*`
//! exactly as the renderer-agnostic core (e.g. `common/renderer/src/
//! view/iterators.rs`) calls it.

#![allow(missing_docs)]

use crate::dom::layout::CocoaBackend;
use crate::dom::Renderer as CocoaRenderer;
use renderer::{renderer::Renderer as RendererTrait, view::Mountable, LayoutBackend};

// Re-export the concrete tree types under the names tachys/leptos/
// the platform expects. `Text` and `Placeholder` are aliases for
// `Element` — the renderer trait wants distinct associated types,
// but on native they're all NSView-backed Elements; the only thing
// distinguishing a "text node" or "placeholder" from a regular
// Element is the NSView subclass + default style applied at creation.
pub use crate::dom::{
    ClassList, CocoaNode, CssStyleDeclaration, Event, TemplateElement,
};


pub type Text = CocoaNode;
pub type Placeholder = CocoaNode;

/// The Cocoa renderer surface — implements [`renderer::Renderer`].
///
/// All methods forward to [`cocoa_dom::Renderer`]; this type exists
/// so we can attach the trait impl + the additional `synthesise_parent_*`
/// helpers below.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dom;

impl RendererTrait for Dom {
    type Backend = CocoaBackend;
    type Node = CocoaNode;

    fn intern(text: &str) -> &str {
        CocoaRenderer::intern(text)
    }

    fn create_text_node(text: &str) -> CocoaNode {
        CocoaRenderer::create_text_node(text)
    }

    fn create_placeholder() -> CocoaNode {
        CocoaRenderer::create_placeholder()
    }

    fn set_text(node: CocoaNode, text: &str) {
        CocoaRenderer::set_text(node, text);
    }

    fn insert_node(
        parent: CocoaNode,
        new_child: CocoaNode,
        anchor: Option<CocoaNode>,
    ) {
        CocoaRenderer::insert_node(parent, new_child, anchor);
    }

    fn remove_node(parent: CocoaNode, child: CocoaNode) -> Option<CocoaNode> {
        CocoaRenderer::remove_node(parent, child)
    }

    fn clear_children(parent: CocoaNode) {
        CocoaRenderer::clear_children(parent);
    }

    fn remove(node: CocoaNode) {
        CocoaRenderer::remove(node);
    }

    fn get_parent(node: CocoaNode) -> Option<CocoaNode> {
        // The parent is a real node in the store; look it up by id.
        // Used by `UnitState::insert_before_this` (the mount anchor
        // for `<Switch>` and other placeholder-based control-flow).
        CocoaBackend::parent(node.id())
            .map(CocoaNode::from_id)
    }

    fn first_child(node: CocoaNode) -> Option<CocoaNode> {
        let _ = node;
        None
    }

    fn next_sibling(node: CocoaNode) -> Option<CocoaNode> {
        let _ = node;
        None
    }

    fn log_node(node: CocoaNode) {
        CocoaRenderer::log_node(node);
    }

    /// Override the trait's default — on cocoa we need to synthesise a
    /// parent Element wrapper around `before`'s superview, with the
    /// right LayoutHandle so the new child registers in the same Taffy
    /// tree.
    #[track_caller]
    fn try_mount_before<M>(new_child: &mut M, before: CocoaNode) -> bool
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
    /// Mount `new_child` immediately before `before`. Panics if `before`
    /// has no parent (mirror of `try_mount_before`).
    #[track_caller]
    pub fn mount_before<M>(new_child: &mut M, before: CocoaNode)
    where
        M: Mountable<Dom>,
    {
        let parent = parent_of(before)
            .expect("Dom::mount_before — node has no parent");
        new_child.mount(parent, Some(before));
    }
}

/// The parent `Node` of `before` in the store, or `None` if `before`
/// is a root (or detached). The parent is a real node; no NSView
/// wrapper synthesis is needed under the thread-local store.
pub(crate) fn parent_of(before: CocoaNode) -> Option<CocoaNode> {
    CocoaBackend::parent(before.id())
        .map(CocoaNode::from_id)
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
    before: CocoaNode,
    child: &mut dyn Mountable<Dom>,
) -> bool {
    let Some(parent) = parent_of(before) else {
        return false;
    };
    child.mount(parent, Some(before));
    true
}

impl Mountable<Dom> for CocoaNode {
    fn unmount(&mut self) {
        self.teardown();
    }

    fn mount(&mut self, parent: CocoaNode, marker: Option<CocoaNode>) {
        <Dom as RendererTrait>::insert_node(parent, *self, marker);
    }

    fn try_mount(
        &mut self,
        parent: CocoaNode,
        marker: Option<CocoaNode>,
    ) -> bool {
        CocoaRenderer::try_insert_node(parent, *self, marker)
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<Dom>) -> bool {
        insert_before_node(*self, child)
    }

    fn elements(&self) -> Vec<CocoaNode> {
        vec![self.clone()]
    }
}

// CastFrom impls live in cocoa_dom (orphan rule: both the trait and
// the types must satisfy locality, and CastFrom doesn't mention Dom,
// so the impls have to sit with Node/Element/Text/Placeholder).
