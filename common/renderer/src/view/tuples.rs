//! `Render<R>` for the unit type and tuples up to size 16.

use super::{Mountable, Render};
use crate::renderer::{CastFrom, Renderer};

/// Retained state for `()` — a placeholder node so insertion points stay
/// stable when an Option/Either branch flips between content and empty.
pub struct UnitState<R: Renderer> {
    placeholder: R::Placeholder,
}

impl<R: Renderer> Render<R> for () {
    type State = UnitState<R>;

    fn build(self) -> Self::State {
        UnitState { placeholder: R::create_placeholder() }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

impl<R: Renderer> Mountable<R> for UnitState<R> {
    fn unmount(&mut self) {
        R::remove(self.placeholder.as_ref());
    }
    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        R::insert_node(parent, self.placeholder.as_ref(), marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        if let Some(parent) = R::get_parent(self.placeholder.as_ref())
            .and_then(R::Element::cast_from)
        {
            child.mount(&parent, Some(self.placeholder.as_ref()));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<R::Element> {
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
            fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
                $( self.$idx.mount(parent, marker); )+
            }
            fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
                $( if self.$idx.insert_before_this(child) { return true; } )+
                false
            }
            fn elements(&self) -> Vec<R::Element> {
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
