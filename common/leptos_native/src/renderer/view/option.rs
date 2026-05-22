//! `Render<R>` for `Option<T>`. State is wrapped in `Either<T::State, ()>`
//! semantics implicitly: when `Some`, the inner T's State; when `None`, no
//! state.

use super::{Mountable, Render};
use crate::renderer::Renderer;

/// Retained state for `Option<T>` — just the inner view's state.
pub struct OptionState<St> {
    inner: Option<St>,
}

impl<R, T> Render<R> for Option<T>
where
    R: Renderer,
    T: Render<R>,
{
    type State = OptionState<T::State>;

    fn build(self) -> Self::State {
        OptionState {
            inner: self.map(|v| v.build()),
        }
    }

    fn rebuild(self, state: &mut Self::State) {
        // `Option<T>` has a long-standing transition gap: going
        // None → Some builds the new state but has no anchor to
        // mount it (the old state was None — no view exists in
        // the tree to position relative to). For control-flow
        // primitives that need to toggle between rendered and
        // nothing-rendered, use `Either<T, ()>` (where `()` builds
        // a Placeholder that serves as the mount anchor) instead
        // of `Option<T>`. `<Show>`, `<Switch>`, `<ShowLet>` all
        // follow this pattern.
        match (self, state.inner.as_mut()) {
            (Some(new), Some(s)) => new.rebuild(s),
            (Some(new), None) => state.inner = Some(new.build()),
            (None, Some(s)) => {
                s.unmount();
                state.inner = None;
            }
            (None, None) => {}
        }
    }
}

impl<R, St> Mountable<R> for OptionState<St>
where
    R: Renderer,
    St: Mountable<R>,
{
    fn unmount(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.unmount();
        }
    }

    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        if let Some(inner) = &mut self.inner {
            inner.mount(parent, marker);
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.inner
            .as_ref()
            .map(|inner| inner.insert_before_this(child))
            .unwrap_or(false)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.inner.as_ref().map(Mountable::elements).unwrap_or_default()
    }
}

impl<R, T> Mountable<R> for Option<T>
where
    R: Renderer,
    T: Mountable<R>,
{
    fn unmount(&mut self) {
        if let Some(inner) = self {
            inner.unmount();
        }
    }

    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        if let Some(inner) = self {
            inner.mount(parent, marker);
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.as_ref()
            .map(|inner| inner.insert_before_this(child))
            .unwrap_or(false)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.as_ref().map(Mountable::elements).unwrap_or_default()
    }
}
