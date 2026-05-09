//! Bridge between `Render<R>` and `reactive_graph` — closures of type
//! `FnMut() -> T` (where `T: Render<R>`) become reactive children: each call
//! happens inside a `RenderEffect`, and the effect re-runs when any signal
//! the closure read changes.

mod owned;
pub use owned::{OwnedView, OwnedViewState};

use crate::{
    renderer::Renderer,
    view::{AddAnyAttr, ApplyAttr, Mountable, Render},
};
use reactive_graph::effect::RenderEffect;
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

/// A reactive view function. Implemented for any `FnMut() -> T + Send +
/// 'static`, so any closure that reads from signals can be used as a child
/// in a view.
pub trait ReactiveFunction: Send + 'static {
    /// The closure's return type.
    type Output;

    /// Calls the closure.
    fn invoke(&mut self) -> Self::Output;
}

impl<F, T> ReactiveFunction for F
where
    F: FnMut() -> T + Send + 'static,
{
    type Output = T;

    fn invoke(&mut self) -> Self::Output {
        self()
    }
}

impl<T: 'static> ReactiveFunction for Arc<Mutex<dyn FnMut() -> T + Send>> {
    type Output = T;

    fn invoke(&mut self) -> Self::Output {
        let mut fun = self.lock().expect("lock poisoned");
        fun()
    }
}

impl<R, F, V> Render<R> for F
where
    R: Renderer,
    F: ReactiveFunction<Output = V>,
    V: Render<R>,
    V::State: 'static,
{
    type State = RenderEffectState<V::State, R>;

    #[track_caller]
    fn build(mut self) -> Self::State {
        RenderEffect::new(move |prev| {
            let value = self.invoke();
            if let Some(mut state) = prev {
                value.rebuild(&mut state);
                state
            } else {
                value.build()
            }
        })
        .into()
    }

    #[track_caller]
    fn rebuild(self, state: &mut Self::State) {
        let new = self.build();
        let mut old = std::mem::replace(state, new);
        old.insert_before_this(state);
        old.unmount();
    }
}

/// Retained view state for a reactive closure. Holds a `RenderEffect<T>`;
/// dropping the state drops the effect, which auto-cancels the subscription.
pub struct RenderEffectState<T: 'static, R> {
    inner: Option<RenderEffect<T>>,
    _phantom: PhantomData<R>,
}

impl<T, R> From<RenderEffect<T>> for RenderEffectState<T, R> {
    fn from(value: RenderEffect<T>) -> Self {
        Self {
            inner: Some(value),
            _phantom: PhantomData,
        }
    }
}

impl<T, R> Mountable<R> for RenderEffectState<T, R>
where
    T: Mountable<R> + 'static,
    R: Renderer,
{
    fn unmount(&mut self) {
        if let Some(inner) = &self.inner {
            inner.with_value_mut(|t| t.unmount());
        }
    }

    fn mount(&mut self, parent: &R::Element, marker: Option<&R::Node>) {
        if let Some(inner) = &self.inner {
            inner.with_value_mut(|t| t.mount(parent, marker));
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        match &self.inner {
            Some(inner) => inner
                .with_value_mut(|t| t.insert_before_this(child))
                .unwrap_or(false),
            None => false,
        }
    }

    fn elements(&self) -> Vec<R::Element> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.with_value_mut(|t| t.elements()))
            .unwrap_or_default()
    }
}

/// `AddAnyAttr<R>` for reactive closures: deferred (branching).
/// `<Show on:click=…>` would route here; supporting it properly
/// needs re-attach-on-rebuild semantics (what to do when the
/// closure swaps content? does the handler stay attached?). For
/// now the attribute is silently dropped.
impl<R, F, V> AddAnyAttr<R> for F
where
    R: Renderer,
    F: ReactiveFunction<Output = V>,
    V: Render<R>,
{
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        self
    }
}
