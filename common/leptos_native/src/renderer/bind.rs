//! Two-way binding plumbing shared across ports.
//!
//! [`IntoSignal<T>`] erases a reactive source — an [`RwSignal<T>`] or a
//! `(getter, setter)` tuple — into boxed get/set closures. Each port's
//! `bind:value` / `bind:checked` installers are then generic over the
//! source instead of re-declaring this trait. The trait is
//! renderer-agnostic (only `reactive_graph`), so it lives here once.

use reactive_graph::{
    signal::RwSignal,
    traits::{Get, Set},
};

/// Conversion from a reactive source into type-erased get/set closures.
///
/// Implemented for [`RwSignal<T>`] (read+write the signal) and for a
/// `(G, S)` tuple of `G: Fn() -> T` + `S: FnMut(T)` (split / derived
/// controllers where there's no single `RwSignal`).
pub trait IntoSignal<T: Send + Sync + 'static>: 'static {
    /// A getter that reads the current value (subscribes to changes
    /// when called inside an Effect).
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static>;
    /// A setter that updates the underlying source.
    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static>;
}

impl<T> IntoSignal<T> for RwSignal<T>
where
    T: Send + Sync + Clone + 'static,
{
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static> {
        let s = *self;
        Box::new(move || s.get())
    }

    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static> {
        let s = *self;
        Box::new(move |v: T| s.set(v))
    }
}

impl<T, G, S> IntoSignal<T> for (G, S)
where
    T: Send + Sync + 'static,
    G: Fn() -> T + Clone + Send + 'static,
    S: FnMut(T) + Clone + Send + 'static,
{
    fn into_get(&self) -> Box<dyn Fn() -> T + Send + 'static> {
        let g = self.0.clone();
        Box::new(move || g())
    }

    fn into_set(&self) -> Box<dyn FnMut(T) + Send + 'static> {
        let mut s = self.1.clone();
        Box::new(move |v: T| s(v))
    }
}
