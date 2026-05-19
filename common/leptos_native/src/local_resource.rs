//! `LocalResource<T>` — a reactive container for a value computed
//! from a non-`Send` async closure.
//!
//! Thin newtype over `AsyncDerived<T, LocalStorage>` plus a
//! convenience `new` constructor and an `IntoFuture` impl that
//! delegates to the inner. The constructor relaxes the `Send + Sync`
//! bound on the future-producing closure that `AsyncDerived::new`
//! requires — useful when the closure captures non-`Send` state
//! (UI signals, NSObject handles, etc.). The reactive graph runs
//! on the main thread on every native port, so the relaxation is
//! safe.
//!
//! Pair with [`Suspend`](crate::suspend::Suspend) to render a
//! view that waits for the resource to resolve.
//!
//! ```ignore
//! let facts = LocalResource::new(move || fetch_facts(count()));
//!
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
//! The name matches SolidJS's `createResource` family; upstream
//! Leptos calls the same type `LocalResource` too.

use reactive_graph::{
    computed::{AsyncDerived, AsyncDerivedFuture, ScopedFuture},
    owner::LocalStorage,
};
use std::future::{Future, IntoFuture};

/// Reactive container for a value computed from an async closure.
/// The closure isn't required to be `Send`, so it can capture
/// non-`Send` UI state. See module docs for usage.
#[derive(Debug)]
pub struct LocalResource<T: 'static> {
    inner: AsyncDerived<T, LocalStorage>,
}

impl<T: 'static> Clone for LocalResource<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner }
    }
}

impl<T: 'static> Copy for LocalResource<T> {}

impl<T: 'static> LocalResource<T> {
    /// Build a `LocalResource` from a closure that produces a
    /// future. The closure re-runs (re-fetching the resource)
    /// whenever a signal it reads via the reactive graph changes.
    #[track_caller]
    pub fn new<Fut>(fun: impl Fn() -> Fut + 'static) -> Self
    where
        Fut: Future<Output = T> + 'static,
    {
        // Wrap in ScopedFuture so the future re-tracks its own
        // reactive scope (matches AsyncDerived::new's behaviour).
        let fun = move || ScopedFuture::new(fun());
        Self {
            inner: AsyncDerived::new_unsync(fun),
        }
    }

    /// Access the inner `AsyncDerived` for `Get` / `With` /
    /// `read` patterns — `resource.inner().get()` returns
    /// `Option<T>` (None while loading).
    pub fn inner(self) -> AsyncDerived<T, LocalStorage> {
        self.inner
    }
}

impl<T> IntoFuture for LocalResource<T>
where
    T: Clone + 'static,
{
    type Output = T;
    type IntoFuture = AsyncDerivedFuture<T>;

    #[track_caller]
    fn into_future(self) -> Self::IntoFuture {
        self.inner.into_future()
    }
}
