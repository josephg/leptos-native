//! Bridge between `Render<R>` and `reactive_graph` — closures of type
//! `FnMut() -> T` (where `T: Render<R>`) become reactive children: each call
//! happens inside a `RenderEffect`, and the effect re-runs when any signal
//! the closure read changes.

mod owned;
pub use owned::{OwnedView, OwnedViewState};

use crate::{
    layout::TreeRef,
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
    fn build(mut self, tree: &TreeRef<R::Backend>) -> Self::State {
        // Capture the tree so the RenderEffect closure (which re-runs
        // on each signal change) can build new states with it.
        let tree_for_effect = tree.clone();
        let effect = RenderEffect::new(move |prev| {
            let value = self.invoke();
            if let Some(mut state) = prev {
                value.rebuild(&mut state);
                state
            } else {
                value.build(&tree_for_effect)
            }
        });
        RenderEffectState {
            tree: send_wrapper::SendWrapper::new(tree.clone()),
            inner: Some(effect),
            _phantom: PhantomData,
        }
    }

    #[track_caller]
    fn rebuild(self, state: &mut Self::State) {
        let tree = (*state.tree).clone();
        let new = self.build(&tree);
        let mut old = std::mem::replace(state, new);
        old.insert_before_this(state);
        old.unmount();
    }
}

/// Retained view state for a reactive closure. Holds a `RenderEffect<T>`;
/// dropping the state drops the effect, which auto-cancels the subscription.
pub struct RenderEffectState<T: 'static, R: Renderer> {
    /// `SendWrapper` so the state is `Send + Sync` (TreeRef = Rc is
    /// not). Native runs single-threaded — the wrapper's panic-on-
    /// off-thread-access is a non-event.
    tree: send_wrapper::SendWrapper<TreeRef<R::Backend>>,
    inner: Option<RenderEffect<T>>,
    _phantom: PhantomData<R>,
}

impl<T: 'static, R: Renderer> RenderEffectState<T, R> {
    /// Construct from a tree + already-built effect. Used by
    /// composite views (e.g. `<ErrorBoundary>`) that need to build
    /// the RenderEffect themselves.
    pub fn from_parts(tree: TreeRef<R::Backend>, effect: RenderEffect<T>) -> Self {
        Self {
            tree: send_wrapper::SendWrapper::new(tree),
            inner: Some(effect),
            _phantom: PhantomData,
        }
    }

    /// Clone the tree this state was built with.
    pub fn tree_ref(&self) -> TreeRef<R::Backend> {
        (*self.tree).clone()
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

/// `AddAnyAttr<R>` for reactive closures (`<Show>`, `<For>`, …).
///
/// Panics. Supporting `<Show on:click=…>` needs re-attach-on-rebuild
/// semantics — when the closure swaps content, does the handler
/// follow? attach to both branches? — that hasn't been designed.
/// Surface the limitation loudly rather than silently dropping the
/// handler.
impl<R, F, V> AddAnyAttr<R> for F
where
    R: Renderer,
    F: ReactiveFunction<Output = V>,
    V: Render<R>,
{
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic!(
            "AddAnyAttr<R>::add_any_attr called on a reactive closure \
             (`<Show>`, `<For>`, etc.). Branching/reactive wrappers \
             aren't supported by the spread machinery yet — it needs \
             re-attach-on-rebuild semantics. Workaround: attach the \
             attribute to the inner element directly, e.g. instead of \
             `<Show on:click=h when=…>{{…}}</Show>`, write \
             `<Show when=…><view on:click=h>…</view></Show>`."
        )
    }
}
