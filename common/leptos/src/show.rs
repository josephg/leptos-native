use crate::{
    children::TypedChildrenFn,
    into_view::{IntoView, View},
};
use either_of::Either;
use leptos_macro::component;
use reactive_graph::{computed::ArcMemo, traits::Get};
use renderer::renderer::Renderer;
use std::{marker::PhantomData, sync::Arc};

/// Function-shaped fallback for `<Show>`. Wraps a `Fn() -> View<Fb>`
/// closure; the prop signature stays clean (`fallback=move || view!{...}`)
/// while preserving the inner Fb type for the renderer.
///
/// Default is a no-op fallback that produces `View<()>`.
pub struct FallbackFn<Fb, R: Renderer> {
    f: Arc<dyn Fn() -> View<Fb> + Send + Sync + 'static>,
    _marker: PhantomData<R>,
}

impl<Fb, R: Renderer> Clone for FallbackFn<Fb, R> {
    fn clone(&self) -> Self {
        Self {
            f: Arc::clone(&self.f),
            _marker: PhantomData,
        }
    }
}

impl<Fb, R: Renderer> FallbackFn<Fb, R> {
    /// Calls the fallback to produce its view.
    pub fn run(&self) -> View<Fb> {
        (self.f)()
    }
}

impl<F, Fb, R> From<F> for FallbackFn<Fb, R>
where
    R: Renderer,
    F: Fn() -> Fb + Send + Sync + 'static,
    Fb: IntoView<R>,
{
    fn from(f: F) -> Self {
        Self {
            f: Arc::new(move || f().into_view()),
            _marker: PhantomData,
        }
    }
}

/// A component that renders its children only when `when` returns `true`.
/// Renders the optional `fallback` (or nothing) when `when` is `false`.
#[component(transparent)]
pub fn Show<W, C, F, Fb, R>(
    /// The children rendered whenever the `when` closure returns `true`.
    children: TypedChildrenFn<C, R>,
    /// A closure that determines whether children render.
    when: W,
    /// Optional fallback rendered whenever `when` is `false`. Pass any
    /// `Fn() -> impl IntoView` closure (e.g.
    /// `fallback=|| view!{ <label>"loading"</label> }`).
    #[prop(optional, into)]
    fallback: Option<FallbackFn<Fb, R>>,
    /// Marker for unused generics. Ignore.
    #[prop(optional)]
    _marker: PhantomData<F>,
) -> impl IntoView<R>
where
    R: Renderer,
    W: Fn() -> bool + Send + Sync + 'static,
    C: IntoView<R> + 'static,
    F: 'static,
    Fb: IntoView<R> + 'static,
{
    let memoized_when = ArcMemo::new(move |_| when());
    let children = children.into_inner();

    move || match memoized_when.get() {
        true => Either::Left(Either::Left(children())),
        false => match &fallback {
            Some(f) => Either::Left(Either::Right(f.run())),
            None => Either::Right(()),
        },
    }
}
