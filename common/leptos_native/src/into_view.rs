use crate::renderer::{
    Backend,
    view::{AddAnyAttr, ApplyAttr, Render},
};
use std::borrow::Cow;

/// A wrapper for any kind of view.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct View<T>
where
    T: Sized,
{
    inner: T,
    #[cfg(debug_assertions)]
    view_marker: Option<Cow<'static, str>>,
}

impl<T> View<T> {
    /// Wraps the view.
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            #[cfg(debug_assertions)]
            view_marker: None,
        }
    }

    /// Unwraps the view, returning the inner type.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Adds a view marker, used for hot-reloading and debug purposes.
    #[inline(always)]
    pub fn with_view_marker(
        #[allow(unused_mut)] mut self,
        #[allow(unused_variables)] view_marker: impl Into<Cow<'static, str>>,
    ) -> Self {
        #[cfg(debug_assertions)]
        {
            self.view_marker = Some(view_marker.into());
        }
        self
    }
}

/// A trait that is implemented for types that can be rendered.
///
/// Generic over the renderer backend `R`. Each platform's `leptos_<platform>`
/// crate uses this with its own `Backend` impl.
///
/// Has `AddAnyAttr<R>` as a supertrait so the macro-emitted
/// `MyComponent(props).add_any_attr(...)` call resolves on a
/// component's opaque `impl IntoView` return type without the user
/// needing the trait in scope explicitly.
pub trait IntoView<R: Backend>
where
    Self: Sized + Render<R> + AddAnyAttr<R> + Send,
{
    /// Wraps the inner type.
    fn into_view(self) -> View<Self>;
}

impl<R, T> IntoView<R> for T
where
    R: Backend,
    T: Sized + Render<R> + AddAnyAttr<R> + Send,
{
    fn into_view(self) -> View<Self> {
        View {
            inner: self,
            #[cfg(debug_assertions)]
            view_marker: None,
        }
    }
}

impl<R: Backend, T: Render<R>> Render<R> for View<T> {
    type State = T::State;

    fn build(self) -> Self::State {
        self.inner.build()
    }

    fn rebuild(self, state: &mut Self::State) {
        self.inner.rebuild(state)
    }
}

/// `View<T>` forwards `.add_any_attr` to its wrapped value. This is
/// the entry point the leptos_macro emits component-spread attrs on:
/// `<MyComponent on:click=…>` becomes
/// `MyComponent(props).add_any_attr((on_click_attr,))`, and
/// `MyComponent(props)` returns a `View<...>` from `into_view()`.
impl<R: Backend, T: AddAnyAttr<R>> AddAnyAttr<R> for View<T> {
    fn add_any_attr<A: ApplyAttr<R>>(self, attr: A) -> Self {
        View {
            inner: self.inner.add_any_attr(attr),
            #[cfg(debug_assertions)]
            view_marker: self.view_marker,
        }
    }
}

/// Collects some iterator of views into a list, so they can be rendered.
pub trait CollectView<R: Backend> {
    /// The inner view type.
    type View: IntoView<R>;

    /// Collects the iterator into a list of views.
    fn collect_view(self) -> Vec<Self::View>;
}

impl<R, It, V> CollectView<R> for It
where
    R: Backend,
    It: IntoIterator<Item = V>,
    V: IntoView<R>,
{
    type View = V;

    fn collect_view(self) -> Vec<Self::View> {
        self.into_iter().collect()
    }
}
