//! `Render<R>` for `Either<A, B>` and `EitherOf3..16` from the `either_of`
//! crate. Used by the `<Show>` / `<Match>` style components and by
//! Option<T>'s state lowering.

use super::{Mountable, Render};
use crate::renderer::Renderer;
use either_of::*;

/// Wrapper around an `Either*` state.
pub struct EitherState<S> {
    inner: S,
}

impl<R: Renderer, S: Mountable<R>> Mountable<R> for EitherState<S> {
    fn unmount(&mut self) {
        self.inner.unmount();
    }
    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        self.inner.mount(parent, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.inner.insert_before_this(child)
    }
    fn elements(&self) -> Vec<R::Node> {
        self.inner.elements()
    }
}

impl<R, A, B> Render<R> for Either<A, B>
where
    R: Renderer,
    A: Render<R>,
    B: Render<R>,
{
    type State = EitherState<Either<A::State, B::State>>;

    fn build(self) -> Self::State {
        let inner = match self {
            Either::Left(a) => Either::Left(a.build()),
            Either::Right(b) => Either::Right(b.build()),
        };
        EitherState { inner }
    }

    fn rebuild(self, state: &mut Self::State) {
                match (self, &mut state.inner) {
            (Either::Left(new), Either::Left(old)) => new.rebuild(old),
            (Either::Right(new), Either::Right(old)) => new.rebuild(old),
            (Either::Right(new), Either::Left(old)) => {
                let mut new_state = new.build();
                old.insert_before_this(&mut new_state);
                old.unmount();
                state.inner = Either::Right(new_state);
            }
            (Either::Left(new), Either::Right(old)) => {
                let mut new_state = new.build();
                old.insert_before_this(&mut new_state);
                old.unmount();
                state.inner = Either::Left(new_state);
            }
        }
    }
}

impl<R, A, B> Mountable<R> for Either<A, B>
where
    R: Renderer,
    A: Mountable<R>,
    B: Mountable<R>,
{
    fn unmount(&mut self) {
        match self {
            Either::Left(a) => a.unmount(),
            Either::Right(b) => b.unmount(),
        }
    }
    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        match self {
            Either::Left(a) => a.mount(parent, marker),
            Either::Right(b) => b.mount(parent, marker),
        }
    }
    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        match self {
            Either::Left(a) => a.insert_before_this(child),
            Either::Right(b) => b.insert_before_this(child),
        }
    }
    fn elements(&self) -> Vec<R::Node> {
        match self {
            Either::Left(a) => a.elements(),
            Either::Right(b) => b.elements(),
        }
    }
}

macro_rules! impl_either_of {
    ($name:ident, $($var:ident),+) => {
        impl<R, $($var),+> Render<R> for $name<$($var),+>
        where
            R: Renderer,
            $($var: Render<R>,)+
        {
            type State = EitherState<$name<$($var::State),+>>;

            fn build(self) -> Self::State {
                let inner = match self {
                    $( $name::$var(v) => $name::$var(v.build()), )+
                };
                EitherState { inner }
            }

            fn rebuild(self, state: &mut Self::State) {
                                match (self, &mut state.inner) {
                    $(
                        ($name::$var(new), $name::$var(old)) => new.rebuild(old),
                    )+
                    (new, inner_state) => {
                        let mut new_state = match new {
                            $( $name::$var(v) => $name::$var(v.build()), )+
                        };
                        inner_state.insert_before_this(&mut new_state);
                        inner_state.unmount();
                        *inner_state = new_state;
                    }
                }
            }
        }

        impl<R, $($var),+> Mountable<R> for $name<$($var),+>
        where
            R: Renderer,
            $($var: Mountable<R>,)+
        {
            fn unmount(&mut self) {
                match self { $( $name::$var(v) => v.unmount(), )+ }
            }
            fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
                match self { $( $name::$var(v) => v.mount(parent, marker), )+ }
            }
            fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
                match self { $( $name::$var(v) => v.insert_before_this(child), )+ }
            }
            fn elements(&self) -> Vec<R::Node> {
                match self { $( $name::$var(v) => v.elements(), )+ }
            }
        }
    };
}

impl_either_of!(EitherOf3, A, B, C);
impl_either_of!(EitherOf4, A, B, C, D);
impl_either_of!(EitherOf5, A, B, C, D, E);
impl_either_of!(EitherOf6, A, B, C, D, E, F);
impl_either_of!(EitherOf7, A, B, C, D, E, F, G);
impl_either_of!(EitherOf8, A, B, C, D, E, F, G, H);
impl_either_of!(EitherOf9, A, B, C, D, E, F, G, H, I);
impl_either_of!(EitherOf10, A, B, C, D, E, F, G, H, I, J);
impl_either_of!(EitherOf11, A, B, C, D, E, F, G, H, I, J, K);
impl_either_of!(EitherOf12, A, B, C, D, E, F, G, H, I, J, K, L);
impl_either_of!(EitherOf13, A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_either_of!(EitherOf14, A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_either_of!(EitherOf15, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_either_of!(EitherOf16, A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);
