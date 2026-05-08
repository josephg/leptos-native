use crate::{
    children::TypedChildrenFn,
    into_view::IntoView,
};
use either_of::Either;
use leptos_macro::component;
use reactive_graph::{computed::ArcMemo, traits::Get};
use renderer::renderer::Renderer;

/// A component that renders its children only when `when` returns `true`.
///
/// Phase 7B: the `fallback` prop from upstream's `<Show>` is not yet
/// supported here. It used a type-erased `ViewFn` (backed by `AnyView`).
/// Phase 8 will re-add it as a typed `TypedChildrenFn<Fb, R>`-shaped
/// prop. For now, invert your condition with a sibling `<Show>` if you
/// need a fallback branch.
#[component(transparent)]
pub fn Show<W, C, R>(
    /// The children rendered whenever the `when` closure returns `true`.
    children: TypedChildrenFn<C, R>,
    /// A closure that determines whether children render.
    when: W,
) -> impl IntoView<R>
where
    R: Renderer,
    W: Fn() -> bool + Send + Sync + 'static,
    C: IntoView<R> + 'static,
{
    let memoized_when = ArcMemo::new(move |_| when());
    let children = children.into_inner();

    move || match memoized_when.get() {
        true => Either::Left(children()),
        false => Either::Right(()),
    }
}
