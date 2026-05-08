//! Component `children` prop types.
//!
//! Phase 7B: pared down from upstream. The web-side variants
//! (`Children = Box<dyn FnOnce() -> AnyView>`, `ChildrenFn`,
//! `ChildrenFragment`, `ChildrenFragmentFn`, etc.) all required
//! `AnyView` — an 881-line type-erasure machine that's tightly bound
//! to RenderHtml/AddAnyAttr. Native components don't need erasure
//! because each application binary has exactly one renderer; we can
//! preserve concrete view types through the typed variants.
//!
//! What survives: `ToChildren` trait + `TypedChildren<T, R>` /
//! `TypedChildrenMut<T, R>` / `TypedChildrenFn<T, R>`. Plus
//! `ChildrenOptContainer<T>` for the macro's optimised path.

use crate::into_view::{IntoView, View};
use renderer::renderer::Renderer;
use std::{
    fmt::{self, Debug},
    marker::PhantomData,
    sync::Arc,
};

/// This trait can be used when constructing a component that takes children
/// without needing to know exactly what children type the component expects.
/// This is used internally by the `view!` macro implementation, and can also
/// be used explicitly when using the builder syntax.
pub trait ToChildren<F> {
    /// Convert the provided type (generally a closure) to Self.
    fn to_children(f: F) -> Self;
}

/// Compiler optimisation, can be used with certain types to avoid unique
/// closures in the `view!{}` macro.
pub struct ChildrenOptContainer<T>(pub T);

/// A typed `children` prop, called once. `T` is the concrete view type
/// the children closure returns, `R` is the renderer.
pub struct TypedChildren<T, R: Renderer> {
    inner: Box<dyn FnOnce() -> View<T> + Send>,
    rndr: PhantomData<R>,
}

impl<T, R: Renderer> TypedChildren<T, R> {
    /// Extracts the inner `children` function.
    pub fn into_inner(self) -> impl FnOnce() -> View<T> + Send {
        self.inner
    }
}

impl<F, C, R> ToChildren<F> for TypedChildren<C, R>
where
    R: Renderer,
    F: FnOnce() -> C + Send + 'static,
    C: IntoView<R>,
{
    #[inline]
    fn to_children(f: F) -> Self {
        TypedChildren {
            inner: Box::new(move || f().into_view()),
            rndr: PhantomData,
        }
    }
}

impl<T, R> ToChildren<ChildrenOptContainer<T>> for TypedChildren<T, R>
where
    R: Renderer,
    T: IntoView<R> + 'static,
{
    #[inline]
    fn to_children(t: ChildrenOptContainer<T>) -> Self {
        TypedChildren {
            inner: Box::new(move || t.0.into_view()),
            rndr: PhantomData,
        }
    }
}

/// A typed `children` prop that may mutate state across calls.
pub struct TypedChildrenMut<T, R: Renderer> {
    inner: Box<dyn FnMut() -> View<T> + Send>,
    rndr: PhantomData<R>,
}

impl<T, R: Renderer> Debug for TypedChildrenMut<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TypedChildrenMut").finish()
    }
}

impl<T, R: Renderer> TypedChildrenMut<T, R> {
    /// Extracts the inner `children` function.
    pub fn into_inner(self) -> impl FnMut() -> View<T> + Send {
        self.inner
    }
}

impl<F, C, R> ToChildren<F> for TypedChildrenMut<C, R>
where
    R: Renderer,
    F: FnMut() -> C + Send + 'static,
    C: IntoView<R>,
{
    #[inline]
    fn to_children(mut f: F) -> Self {
        TypedChildrenMut {
            inner: Box::new(move || f().into_view()),
            rndr: PhantomData,
        }
    }
}

impl<T, R> ToChildren<ChildrenOptContainer<T>> for TypedChildrenMut<T, R>
where
    R: Renderer,
    T: IntoView<R> + Clone + 'static,
{
    #[inline]
    fn to_children(t: ChildrenOptContainer<T>) -> Self {
        TypedChildrenMut {
            inner: Box::new(move || t.0.clone().into_view()),
            rndr: PhantomData,
        }
    }
}

/// A typed `children` prop that can be called many times (e.g. for `<Show>`,
/// `<Suspense>`).
pub struct TypedChildrenFn<T, R: Renderer> {
    inner: Arc<dyn Fn() -> View<T> + Send + Sync>,
    rndr: PhantomData<R>,
}

impl<T, R: Renderer> Debug for TypedChildrenFn<T, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TypedChildrenFn").finish()
    }
}

impl<T, R: Renderer> Clone for TypedChildrenFn<T, R> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            rndr: PhantomData,
        }
    }
}

impl<T, R: Renderer> TypedChildrenFn<T, R> {
    /// Extracts the inner `children` function.
    pub fn into_inner(self) -> Arc<dyn Fn() -> View<T> + Send + Sync> {
        self.inner
    }
}

impl<F, C, R> ToChildren<F> for TypedChildrenFn<C, R>
where
    R: Renderer,
    F: Fn() -> C + Send + Sync + 'static,
    C: IntoView<R>,
{
    #[inline]
    fn to_children(f: F) -> Self {
        TypedChildrenFn {
            inner: Arc::new(move || f().into_view()),
            rndr: PhantomData,
        }
    }
}

impl<T, R> ToChildren<ChildrenOptContainer<T>> for TypedChildrenFn<T, R>
where
    R: Renderer,
    T: IntoView<R> + Clone + Sync + 'static,
{
    #[inline]
    fn to_children(t: ChildrenOptContainer<T>) -> Self {
        TypedChildrenFn {
            inner: Arc::new(move || t.0.clone().into_view()),
            rndr: PhantomData,
        }
    }
}
