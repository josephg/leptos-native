//! `<Transition>` — coordinate one or more `Suspend`s under a
//! shared fallback.
//!
//! Minimal implementation: renders `children` always. Provides a
//! `fallback` prop that's currently used only as a placeholder
//! the developer can drop in; the actual fallback rendering is
//! per-`Suspend` (each suspend keeps its own placeholder until
//! its future resolves).
//!
//! Upstream Leptos's `<Transition>` additionally coordinates a
//! shared "is-anything-pending" suspense context so a single
//! fallback covers all suspended regions in the subtree, and
//! keeps the previous resolved children visible while a *new*
//! load is in flight (the "transition" behaviour, vs the
//! `<Suspense>` flash-to-fallback semantics). That coordination
//! requires a suspense-context machinery this fork hasn't built
//! out yet — when it lands, `<Transition>` will gain those
//! semantics without an API-shape change.

use crate::{
    children::TypedChildrenFn, into_view::IntoView, show::FallbackFn,
};
use leptos_macro::component;
use renderer::renderer::Renderer;

/// Wrap children that contain one or more `Suspend`s. Renders
/// the children directly today; the `fallback` prop is accepted
/// for forward compatibility but has no rendering effect — each
/// `Suspend` shows its own placeholder until its future resolves.
#[component(transparent)]
pub fn Transition<C, Fb, R>(
    /// The children rendered as soon as they exist. Wrap async
    /// regions in `Suspend::new(...)` to defer their rendering
    /// until a future resolves.
    children: TypedChildrenFn<C, R>,
    /// Currently unused but accepted for API parity with upstream.
    /// Will be the shared loading fallback once the suspense-
    /// context machinery lands.
    #[prop(optional, into)]
    fallback: Option<FallbackFn<Fb, R>>,
) -> impl IntoView<R>
where
    R: Renderer,
    C: IntoView<R> + 'static,
    Fb: IntoView<R> + 'static,
{
    let _ = fallback; // accepted for forward compat; see fn docs
    let children = children.into_inner();
    move || children()
}
