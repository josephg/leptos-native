//! `Render<R>` for the unit type and tuples up to size 16.

use super::{Mountable, Render};
use crate::renderer::Renderer;

/// Retained state for an `Option`/`Either` empty branch — a real
/// placeholder node so insertion points stay stable when the branch
/// flips between content and empty. Constructed explicitly by callers
/// that need a placeholder (see `view::result`); not emitted by
/// `() : Render<R>` itself, which is a no-op.
pub struct UnitState<R: Renderer> {
    placeholder: R::Node,
}

impl<R: Renderer> UnitState<R> {
    /// Build a fresh placeholder-backed unit state in `tree`.
    pub fn new() -> Self {
        UnitState { placeholder: R::create_placeholder() }
    }
}

/// `() : Render<R>` is a NO-OP — building it yields `()` (which is
/// itself `Mountable<R>` as a no-op below) and produces no platform
/// nodes.
///
/// This matters because builder containers (e.g. `vstack().child(c1)`)
/// seed their child accumulator with `()`. If `Render::build` for `()`
/// produced a `UnitState` placeholder, every container in the tree
/// would acquire an extra placeholder NSView underneath it — turning
/// every leaf control (`button`, `label`, etc.) into a non-leaf in
/// Taffy and breaking intrinsic-size measurement.
impl<R: Renderer> Render<R> for () {
    type State = ();
    fn build(self) -> Self::State {}
    fn rebuild(self, _state: &mut Self::State) {}
}

/// `Mountable<R> for ()` — no-op. Used as the `ChildState` for leaf
/// controls (Button, Label, …) that don't have any children. See the
/// comment on `Render for ()` above for why a real placeholder there
/// would break intrinsic-size measurement on the parent control.
impl<R: Renderer> Mountable<R> for () {
    fn unmount(&mut self) {}
    fn mount(&mut self, _parent: &R::Node, _marker: Option<&R::Node>) {}
    fn insert_before_this(&self, _child: &mut dyn Mountable<R>) -> bool {
        false
    }
    fn elements(&self) -> Vec<R::Node> {
        Vec::new()
    }
}

impl<R: Renderer> Mountable<R> for UnitState<R> {
    fn unmount(&mut self) {
        R::remove(&self.placeholder);
    }
    fn mount(&mut self, parent: &R::Node, marker: Option<&R::Node>) {
        R::insert_node(parent, &self.placeholder, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(&self.placeholder)
        {
            child.mount(&parent, Some(&self.placeholder));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Node> {
        Vec::new()
    }
}

macro_rules! impl_render_tuple {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<R: Renderer, $($T),+> Render<R> for ($($T,)+)
        where
            $($T: Render<R>,)+
        {
            type State = ($($T::State,)+);

            fn build(self) -> Self::State {
                ( $( self.$idx.build(), )+ )
            }

            fn rebuild(self, state: &mut Self::State) {
                $( self.$idx.rebuild(&mut state.$idx); )+
            }
        }

        impl<R: Renderer, $($T),+> Mountable<R> for ($($T,)+)
        where
            $($T: Mountable<R>,)+
        {
            fn unmount(&mut self) {
                $( self.$idx.unmount(); )+
            }
            fn mount(&mut self, parent: &R::Node, marker: Option<&R::Node>) {
                $( self.$idx.mount(parent, marker); )+
            }
            fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
                $( if self.$idx.insert_before_this(child) { return true; } )+
                false
            }
            fn elements(&self) -> Vec<R::Node> {
                let mut out = Vec::new();
                $( out.extend(self.$idx.elements()); )+
                out
            }
        }
    };
}

impl_render_tuple!((0, A));
impl_render_tuple!((0, A), (1, B));
impl_render_tuple!((0, A), (1, B), (2, C));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L), (12, M));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L), (12, M), (13, N));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L), (12, M), (13, N), (14, O));
impl_render_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H), (8, I), (9, J), (10, K), (11, L), (12, M), (13, N), (14, O), (15, P));
