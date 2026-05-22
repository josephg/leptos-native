//! `Render<R>` for `Result<T, E>`.
//!
//! On `Err`, the value is thrown into the active `throw_error` hook
//! (set by the nearest `<ErrorBoundary>`); on `Ok` the inner view is
//! rendered as usual. The `State` keeps the throw_error `ErrorId` so
//! the error can be cleared on re-render or drop.

use crate::renderer::{
    Renderer,
    view::{Mountable, Render, UnitState},
};
use either_of::Either;
use std::sync::Arc;
use throw_error::{Error as AnyError, ErrorHook};

/// View state for a `Result<_, _>`.
pub struct ResultState<T, R>
where
    R: Renderer,
    T: Render<R>,
{
    state: Either<T::State, UnitState<R>>,
    error: Option<throw_error::ErrorId>,
    hook: Option<Arc<dyn ErrorHook>>,
}

impl<T, R> Drop for ResultState<T, R>
where
    R: Renderer,
    T: Render<R>,
{
    fn drop(&mut self) {
        // Clear any registered error so the boundary doesn't keep
        // showing it after this branch goes away.
        if let Some(e) = self.error.take() {
            throw_error::clear(&e);
        }
    }
}

impl<R, T, E> Render<R> for Result<T, E>
where
    R: Renderer,
    T: Render<R>,
    E: Into<AnyError> + 'static,
{
    type State = ResultState<T, R>;

    fn build(self) -> Self::State {
        let hook = throw_error::get_error_hook();
        let (state, error) = match self {
            Ok(view) => (Either::Left(view.build()), None),
            Err(e) => (
                // Need a real placeholder here so the slot in the tree
                // is preserved while the error is showing — Render for
                // `()` is now a no-op (see view::tuples).
                Either::Right(UnitState::new()),
                Some(throw_error::throw(e.into())),
            ),
        };
        ResultState { state, error, hook }
    }

    fn rebuild(self, state: &mut Self::State) {
        let _guard = state.hook.clone().map(throw_error::set_error_hook);
        match (&mut state.state, self) {
            // Err -> Err: replace
            (Either::Right(_), Err(new)) => {
                if let Some(old_error) = state.error.take() {
                    throw_error::clear(&old_error);
                }
                state.error = Some(throw_error::throw(new.into()));
            }
            // Ok -> Ok: rebuild child
            (Either::Left(old), Ok(new)) => {
                T::rebuild(new, old);
            }
            // Ok -> Err: unmount old, mount placeholder, throw error
            (Either::Left(old), Err(err)) => {
                let mut new_state = UnitState::<R>::new();
                old.insert_before_this(&mut new_state);
                old.unmount();
                state.state = Either::Right(new_state);
                state.error = Some(throw_error::throw(err.into()));
            }
            // Err -> Ok: clear error, build new view
            (Either::Right(old), Ok(new)) => {
                if let Some(err) = state.error.take() {
                    throw_error::clear(&err);
                }
                let mut new_state = new.build();
                old.insert_before_this(&mut new_state);
                old.unmount();
                state.state = Either::Left(new_state);
            }
        }
    }
}

impl<R, T> Mountable<R> for ResultState<T, R>
where
    R: Renderer,
    T: Render<R>,
{
    fn unmount(&mut self) {
        self.state.unmount();
    }

    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        self.state.mount(parent, marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.state.insert_before_this(child)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.state.elements()
    }
}
