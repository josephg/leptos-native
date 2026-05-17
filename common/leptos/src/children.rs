//! Component `children` prop types.
//!
//! Pared down from upstream. The web-side variants
//! (`Children = Box<dyn FnOnce() -> AnyView>`, `ChildrenFn`,
//! `ChildrenFragment`, `ChildrenFragmentFn`, etc.) all required
//! `AnyView` — an 881-line type-erasure machine. Native components
//! don't need erasure because each application binary has exactly
//! one renderer; we preserve concrete view types through the typed
//! variants.
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

// ---------------------------------------------------------------------
// Type-erased children, built on top of renderer::view::AnyView<R>
// ---------------------------------------------------------------------

use renderer::view::{AnyView, IntoAny};

/// A `children` prop that erases its concrete view type. Use when
/// the slot or component needs to accept arbitrary view shapes —
/// in particular for slot children that vary per call-site, or
/// where dispatch in the component body produces mismatched
/// concrete view types from different branches.
///
/// Trade-off vs `TypedChildrenFn`: one `Box` allocation per child
/// instance; compile-time view-type checking is lost past the
/// erasure point. Prefer `TypedChildrenFn` when all callers
/// produce the same view shape.
///
/// Generic over the renderer `R`; per-port aliases name a
/// concrete `Children = ChildrenFn<Dom>`.
pub struct ChildrenFn<R: Renderer> {
    inner: Arc<dyn Fn() -> AnyView<R> + Send + Sync>,
}

impl<R: Renderer> Clone for ChildrenFn<R> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<R: Renderer> Debug for ChildrenFn<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ChildrenFn").finish()
    }
}

impl<R: Renderer> ChildrenFn<R> {
    /// Call the children closure, producing a fresh `AnyView<R>`.
    pub fn run(&self) -> AnyView<R> {
        (self.inner)()
    }
}

/// Sugar so `children()` works in component bodies (matches the
/// upstream Leptos ergonomics).
impl<R: Renderer> std::ops::Deref for ChildrenFn<R> {
    type Target = dyn Fn() -> AnyView<R> + Send + Sync;
    fn deref(&self) -> &Self::Target {
        &*self.inner
    }
}

impl<F, V, R> ToChildren<F> for ChildrenFn<R>
where
    R: Renderer,
    F: Fn() -> V + Send + Sync + 'static,
    V: Render<R> + Send + 'static,
    V::State: Send + Sync + 'static,
{
    #[inline]
    fn to_children(f: F) -> Self {
        ChildrenFn {
            inner: Arc::new(move || f().into_any()),
        }
    }
}

/// Compiler-optimisation path: when the view macro detects a
/// children expression that's a single Clone value (a string
/// literal, a number, etc.), it wraps it in
/// `ChildrenOptContainer<T>` instead of synthesising a unique
/// closure. This impl lets that path land on `ChildrenFn`.
impl<T, R> ToChildren<ChildrenOptContainer<T>> for ChildrenFn<R>
where
    R: Renderer,
    T: Render<R> + Clone + Send + Sync + 'static,
    T::State: Send + Sync + 'static,
{
    #[inline]
    fn to_children(t: ChildrenOptContainer<T>) -> Self {
        ChildrenFn {
            inner: Arc::new(move || t.0.clone().into_any()),
        }
    }
}

use renderer::view::Render;
