//! `Suspend<F>` — render a future as a view.
//!
//! Wraps a `Future<Output = V>` where `V: Render<R>`. When built,
//! mounts a placeholder; spawns the future on the leptos
//! executor; when the future resolves, builds `V` and swaps the
//! placeholder for `V`'s mounted state in place.
//!
//! Designed for use under a [`Transition`](crate::transition)
//! and with [`LocalResource`](crate::local_resource):
//!
//! ```ignore
//! let facts = LocalResource::new(move || fetch_facts(count()));
//! view! {
//!     <Transition fallback=|| view! { <label>"Loading…"</label> }>
//!         <stack>
//!             {move || Suspend::new(async move {
//!                 facts.await.map(|fs| fs.join("\n\n"))
//!             })}
//!         </stack>
//!     </Transition>
//! }
//! ```
//!
//! Semantics for the minimal version implemented here:
//!
//! - **Build**: mount a placeholder; spawn the future.
//! - **Resolved**: build the inner `V`, splice it in before the
//!   placeholder, then unmount the placeholder.
//! - **Rebuild**: drop the old state (including any
//!   in-flight future via its dropping the boxed task) and
//!   re-run `build`.
//! - **Unmount**: tear down the placeholder or resolved state.
//!
//! ## Trade-offs
//!
//! Each `Suspend` is independent — there's no shared
//! "loading-now" context across multiple `Suspend`s inside the
//! same `<Transition>` yet. Each suspended region shows its own
//! placeholder until that specific future resolves.

use any_spawner::Executor;
use renderer::{
    renderer::Renderer,
    view::{Mountable, Render},
};
use std::{cell::RefCell, future::Future, rc::Rc};

/// Splice `state` into the same position the `placeholder`
/// currently occupies, then unmount the placeholder. Falls back
/// to `mount` against `parent`/`marker` if the placeholder has no
/// real presence to splice before.
fn splice_in_place<R, S>(
    placeholder: &mut R::Node,
    state: &mut S,
    parent: Option<&R::Node>,
    marker: Option<&R::Node>,
) where
    R: Renderer,
    S: Mountable<R>,
{
    let inserted = <R::Node as Mountable<R>>::insert_before_this(
        placeholder,
        state as &mut dyn Mountable<R>,
    );
    if !inserted {
        if let Some(parent) = parent {
            state.mount(parent, marker);
        }
    }
    <R::Node as Mountable<R>>::unmount(placeholder);
}

/// Wraps a future so it can be rendered as a view. Built via
/// `Suspend::new(async { ... })`.
pub struct Suspend<F> {
    future: F,
}

impl<F> Suspend<F> {
    /// Wrap the given future.
    pub fn new(future: F) -> Self {
        Self { future }
    }
}

/// State for a mounted `Suspend`. Internally either a placeholder
/// (while the future is still pending) or the resolved view's
/// state (after the future has resolved).
pub struct SuspendState<R, V>
where
    R: Renderer,
    V: Render<R>,
{
    // Shared so the spawned task can mutate the variant when the
    // future resolves. Single-threaded (main thread) by
    // construction, hence `Rc<RefCell<_>>` is fine.
    inner: Rc<RefCell<SuspendInner<R, V>>>,
}

enum SuspendInner<R, V>
where
    R: Renderer,
    V: Render<R>,
{
    /// Future hasn't resolved yet. We've mounted a placeholder
    /// somewhere; remember where so the future's continuation can
    /// splice the resolved view next to it.
    Pending {
        placeholder: R::Node,
        parent: Option<R::Node>,
        marker: Option<R::Node>,
    },
    /// Future has resolved and the inner view is mounted.
    Ready { state: V::State },
}

impl<F, V, R> Render<R> for Suspend<F>
where
    R: Renderer,
    F: Future<Output = V> + 'static,
    V: Render<R> + 'static,
    V::State: 'static,
{
    type State = SuspendState<R, V>;

    fn build(self) -> Self::State {
        let placeholder = R::create_placeholder();
        let inner = Rc::new(RefCell::new(SuspendInner::Pending {
            placeholder,
            parent: None,
            marker: None,
        }));

        // Spawn the future. When it resolves, splice the resolved
        // view into the same position as the placeholder. We hold
        // a `Weak` rather than a strong clone so the
        // `SuspendState` being dropped before the future resolves
        // doesn't leak resources — if the upgrade fails, we
        // explicitly unmount the freshly-built state instead of
        // dropping it (which on some V::State types would skip
        // RAII cleanup like Effect cancellation).
        let inner_weak = Rc::downgrade(&inner);
        let future = self.future;
        Executor::spawn_local(async move {
            let view = future.await;
            let mut state = view.build();

            let Some(inner) = inner_weak.upgrade() else {
                // SuspendState was dropped while the future was in
                // flight. Tear down what we just built.
                state.unmount();
                return;
            };
            let mut guard = inner.borrow_mut();
            match &mut *guard {
                SuspendInner::Pending { placeholder, parent, marker } => {
                    splice_in_place::<R, _>(
                        placeholder,
                        &mut state,
                        parent.as_ref(),
                        marker.as_ref(),
                    );
                    *guard = SuspendInner::Ready { state };
                }
                SuspendInner::Ready { .. } => {
                    // Defensive — shouldn't happen with one future
                    // per build. Tear down the freshly-built state
                    // so its RAII drops run with a mounted parent.
                    state.unmount();
                }
            }
        });

        SuspendState { inner }
    }

    fn rebuild(self, state: &mut Self::State) {
        // Drop the previous state (its placeholder / resolved
        // mount gets unmounted via Drop) and build fresh.
        // Note: this kills any in-flight future from the previous
        // build — its continuation will see Ready or a dropped
        // inner and bail.
        let new_state = self.build();
        // Replace contents. The previous SuspendState's Drop
        // (via the Rc-held Pending/Ready) doesn't run here — we
        // need explicit unmount.
        state.unmount();
        *state = new_state;
    }
}

impl<R, V> Mountable<R> for SuspendState<R, V>
where
    R: Renderer,
    V: Render<R>,
{
    fn unmount(&mut self) {
        let mut guard = self.inner.borrow_mut();
        match &mut *guard {
            SuspendInner::Pending { placeholder, parent, marker } => {
                placeholder.unmount();
                *parent = None;
                *marker = None;
            }
            SuspendInner::Ready { state } => {
                state.unmount();
            }
        }
    }

    fn mount(&mut self, parent: &R::Node, marker: Option<&R::Node>) {
        let mut guard = self.inner.borrow_mut();
        match &mut *guard {
            SuspendInner::Pending { placeholder, parent: p, marker: m } => {
                placeholder.mount(parent, marker);
                *p = Some(parent.clone());
                *m = marker.cloned();
            }
            SuspendInner::Ready { state } => {
                state.mount(parent, marker);
            }
        }
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        let guard = self.inner.borrow();
        match &*guard {
            SuspendInner::Pending { placeholder, .. } => {
                placeholder.insert_before_this(child)
            }
            SuspendInner::Ready { state } => {
                state.insert_before_this(child)
            }
        }
    }

    fn elements(&self) -> Vec<R::Node> {
        let guard = self.inner.borrow();
        match &*guard {
            SuspendInner::Pending { placeholder, .. } => {
                placeholder.elements()
            }
            SuspendInner::Ready { state } => state.elements(),
        }
    }
}
