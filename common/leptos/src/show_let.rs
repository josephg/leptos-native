use crate::into_view::IntoView;
use either_of::Either;
use leptos_macro::component;
#[cfg(not(all(feature = "nightly", rustc_nightly)))]
use reactive_graph::traits::Get;
use renderer::renderer::Renderer;
use std::{marker::PhantomData, sync::Arc};

/// Like `<Show>`, but for `Option`. Shows children when `some` returns `Some`.
///
/// Note: upstream's `<ShowLet>` had a `fallback` prop backed by
/// `ViewFn`/`AnyView`; that's not available in this fork (see `Show`
/// doc for context).
#[component(transparent)]
pub fn ShowLet<T, ChFn, V, M, R>(
    /// Rendered when `some` returns `Some(t)`. Receives `t` as its argument.
    children: ChFn,

    /// A signal or closure that returns an `Option`.
    some: impl IntoOptionGetter<T, M>,

    /// Marker for generic parameters. Ignore this.
    #[prop(optional)]
    _marker: PhantomData<(T, M, R)>,
) -> impl IntoView<R>
where
    R: Renderer,
    ChFn: Fn(T) -> V + Send + Clone + 'static,
    V: IntoView<R> + 'static,
    T: 'static,
{
    let getter = some.into_option_getter();

    move || {
        let children = children.clone();

        getter
            .run()
            .map(move |t| Either::Left(children(t)))
            .unwrap_or(Either::Right(()))
    }
}

/// Wrapper around an `Option`-producing closure or signal.
pub struct OptionGetter<T>(Arc<dyn Fn() -> Option<T> + Send + Sync + 'static>);

impl<T> Clone for OptionGetter<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T> OptionGetter<T> {
    /// Runs the getter and returns the result.
    pub fn run(&self) -> Option<T> {
        (self.0)()
    }
}

/// Conversion trait for creating an `OptionGetter` from a closure or signal.
pub trait IntoOptionGetter<T, M> {
    /// Converts the given value into an `OptionGetter`.
    fn into_option_getter(self) -> OptionGetter<T>;
}

/// Marker type for the closure impl of `IntoOptionGetter`.
pub struct FunctionMarker;

impl<T, F> IntoOptionGetter<T, FunctionMarker> for F
where
    F: Fn() -> Option<T> + Send + Sync + 'static,
{
    fn into_option_getter(self) -> OptionGetter<T> {
        OptionGetter(Arc::new(self))
    }
}

/// Marker type for the signal impl of `IntoOptionGetter`.
///
/// On nightly, signal types implement `Fn() -> T` directly, so they go through
/// the `FunctionMarker` impl instead. This impl is only needed on stable.
pub struct SignalMarker;

#[cfg(not(all(feature = "nightly", rustc_nightly)))]
impl<T, S> IntoOptionGetter<T, SignalMarker> for S
where
    S: Get<Value = Option<T>> + Clone + Send + Sync + 'static,
{
    fn into_option_getter(self) -> OptionGetter<T> {
        let cloned = self.clone();
        OptionGetter(Arc::new(move || cloned.get()))
    }
}

/// Marker type for the `Option<T>` static-value impl of `IntoOptionGetter`.
pub struct StaticMarker;

impl<T> IntoOptionGetter<T, StaticMarker> for Option<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn into_option_getter(self) -> OptionGetter<T> {
        OptionGetter(Arc::new(move || self.clone()))
    }
}
