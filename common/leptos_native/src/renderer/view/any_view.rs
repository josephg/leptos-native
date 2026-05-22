//! `AnyView<R>` — type-erased view container.
//!
//! Wraps an arbitrary `Render<R>` value behind a `Box<dyn …>`,
//! producing a uniform `AnyView<R>` type that any code can hand
//! around without knowing the concrete view shape. Used by:
//!
//!   * `ChildrenFn` and the `#[slot]` macro, when slot children
//!     need to vary per call-site;
//!   * `<Show fallback>` when the children and fallback branches
//!     return different concrete views;
//!   * future `<Transition>` / `<AnimatedShow>` work.
//!
//! Trade-off: `AnyView` allocates (one `Box` per value) and
//! prevents compile-time type checking past the erasure point.
//! Prefer concrete types (`TypedChildren<V, R>`, `Either2<A, B>`)
//! when you know all view shapes statically. Reach for `AnyView`
//! when you specifically need to mix shapes at runtime.
//!
//! ## Rebuild semantics
//!
//! Calling `rebuild` on an `AnyView` always unmounts the old
//! state and builds the new view from scratch. We can't safely
//! re-use the existing state because the concrete inner type may
//! have changed (the whole point of erasure). This is the same
//! tradeoff upstream Leptos makes.

use crate::renderer::{
    Renderer,
    view::{Mountable, Render},
};

/// Type-erased view. Generic over the renderer so per-port code
/// can newtype-alias `AnyView = AnyView<Dom>`.
///
/// Construct via `.into_any()` (extension method on any
/// `Render<R>` value) or `AnyView::new(value)`.
pub struct AnyView<R: Renderer> {
    inner: Box<dyn ErasedRender<R> + Send>,
}

/// Per-instance state held by a mounted `AnyView`. Boxed so the
/// concrete `State` type stays hidden behind the erasure.
///
/// The inner trait object is `Send + Sync` (not just `Send`) so an
/// `AnyViewState` can flow through the reactive bridge: a
/// `RenderEffect<T>` wraps its value in `Arc<RwLock<Option<T>>>`,
/// and `Arc<RwLock<X>>` is only `Send` when `X: Send + Sync`. The
/// closure-returning-`AnyView` pattern used by `<Show>`-style
/// reactive children flows the state through that bridge.
///
/// All native ports keep their per-element state behind
/// `SendWrapper`s — `Send + Sync` for any `Send` payload — so the
/// stricter bound is satisfied in practice.
pub struct AnyViewState<R: Renderer> {
    inner: Box<dyn ErasedMountable<R> + Send + Sync>,
}

impl<R: Renderer> AnyView<R> {
    /// Erase a `Render<R>` into an `AnyView<R>`.
    pub fn new<V>(view: V) -> Self
    where
        V: Render<R> + Send + 'static,
        V::State: Send + Sync + 'static,
    {
        Self {
            inner: Box::new(ErasedRenderImpl(Some(view))),
        }
    }
}

/// Extension trait that adds `.into_any()` to any `Render<R>` so
/// callers can write `view.into_any()` instead of
/// `AnyView::new(view)`. Matches the upstream API shape.
pub trait IntoAny<R: Renderer> {
    /// Erase this view into an `AnyView<R>`.
    fn into_any(self) -> AnyView<R>;
}

impl<R, V> IntoAny<R> for V
where
    R: Renderer,
    V: Render<R> + Send + 'static,
    V::State: Send + Sync + 'static,
{
    fn into_any(self) -> AnyView<R> {
        AnyView::new(self)
    }
}

impl<R: Renderer> Render<R> for AnyView<R> {
    type State = AnyViewState<R>;

    fn build(self) -> Self::State {
        let inner = self.inner.build_erased();
        AnyViewState { inner }
    }

    fn rebuild(self, state: &mut Self::State) {
        // Concrete inner type may have changed — we can't reuse
        // the existing state. Unmount it, build the new view
        // fresh, and splice it in where the old one lived.
        let mut new_state = self.build();
        state.inner.insert_before_this(&mut new_state);
        state.inner.unmount();
        *state = new_state;
    }
}

impl<R: Renderer> Mountable<R> for AnyViewState<R> {
    fn unmount(&mut self) {
        self.inner.unmount();
    }

    fn mount(&mut self, parent: R::Node, marker: Option<R::Node>) {
        self.inner.mount(parent, marker);
    }

    fn insert_before_this(&self, child: &mut dyn Mountable<R>) -> bool {
        self.inner.insert_before_this(child)
    }

    fn elements(&self) -> Vec<R::Node> {
        self.inner.elements()
    }
}

// ---------------------------------------------------------------------
// Internals: erased trait objects
// ---------------------------------------------------------------------

trait ErasedRender<R: Renderer> {
    /// Consume the boxed view and produce a boxed-state value. We
    /// take `Box<Self>` here so the impl can move the inner value
    /// out of `Option` (Rust can't move out of a `Box<dyn …>` by
    /// reference, so the implementation stashes the value in an
    /// `Option<V>` and `.take()`s it).
    fn build_erased(
        self: Box<Self>,
    ) -> Box<dyn ErasedMountable<R> + Send + Sync>;
}

/// Marker trait combining `Mountable<R> + Send + Sync`. Used as
/// the boxed-state type inside `AnyViewState`. Sync is required
/// so that `RenderEffect<AnyViewState<R>>` (which wraps the value
/// in `Arc<RwLock<Option<T>>>`) can satisfy its `Send` bound — see
/// the docstring on `AnyViewState`. Blanket-impl'd for any
/// concrete `M` that satisfies the bounds.
trait ErasedMountable<R: Renderer>: Mountable<R> + Send + Sync {}

impl<R, M> ErasedMountable<R> for M
where
    R: Renderer,
    M: Mountable<R> + Send + Sync + 'static,
{
}

/// Wrapper around a concrete `V: Render<R>` so we can store it
/// behind `Box<dyn ErasedRender<R>>`. The `Option` lets us move
/// `V` out of `&mut self` in `build_erased` without exposing
/// `Self: Sized` requirements.
struct ErasedRenderImpl<V>(Option<V>);

impl<R, V> ErasedRender<R> for ErasedRenderImpl<V>
where
    R: Renderer,
    V: Render<R> + Send + 'static,
    V::State: Send + Sync + 'static,
{
    fn build_erased(
        mut self: Box<Self>,
    ) -> Box<dyn ErasedMountable<R> + Send + Sync> {
        let view = self
            .0
            .take()
            .expect("AnyView::build_erased called twice (internal bug)");
        Box::new(view.build())
    }
}
