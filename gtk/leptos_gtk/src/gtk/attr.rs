//! Attribute-value plumbing for GTK elements.
//!
//! Builder methods like `.title(...)` accept anything that implements
//! [`IntoMaybeReactive<T>`]. The two impls of interest:
//!
//! - **`T` itself** — a static value. Wrapped as `MaybeReactive::Static`.
//! - **`F: Fn() -> T`** — a closure. Wrapped as `MaybeReactive::Reactive`.
//!   At build time we register a [`RenderEffect`] that re-runs the
//!   closure whenever any signal it reads changes, and updates the
//!   underlying GTK widget property each time.
//!
//! The [`MaybeReactive`] enum and the [`install`] driver are
//! re-exports from `renderer-common` (`renderer::attrs`) — they're
//! shared with every native backend. The [`IntoMaybeReactive`] trait,
//! however, is **port-local**: it lives in this module so we can
//! provide impls for GTK / Taffy-foreign types like `FlexDirection`
//! and `JustifyContent` without orphan-rule violations. Renderer-
//! common's `WithLayout` / `WithUniversal` default methods use a
//! separately-defined trait of the same name that only covers
//! renderer-common-owned types (f32, Dim, AlignSelf, …); the two
//! traits coexist because each builder method's bound pins the trait
//! it wants explicitly.

pub use renderer::attrs::{install, AlignSelf, Dim, MaybeReactive};

/// Conversion trait so attribute setters can take either a bare
/// value or a `Fn() -> T` closure transparently. Port-local —
/// see the module docs for why.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

// Static-value impls. `&str` and `String` have explicit impls so
// callers can pass them without `.to_string()`.
impl IntoMaybeReactive<String> for String {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<String> for &str {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self.to_owned())
    }
}

impl IntoMaybeReactive<bool> for bool {
    fn into_maybe_reactive(self) -> MaybeReactive<bool> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<i32> for i32 {
    fn into_maybe_reactive(self) -> MaybeReactive<i32> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<f64> for f64 {
    fn into_maybe_reactive(self) -> MaybeReactive<f64> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<f32> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<f32> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<Dim> for Dim {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<Dim> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(Dim::Px(self))
    }
}

impl<F> IntoMaybeReactive<Dim> for F
where
    F: Fn() -> Dim + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl IntoMaybeReactive<usize> for usize {
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::FlexDirection>
    for gtk_dom::layout::FlexDirection
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexDirection> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::JustifyContent>
    for gtk_dom::layout::JustifyContent
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::JustifyContent> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::AlignItems>
    for gtk_dom::layout::AlignItems
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::AlignItems> {
        MaybeReactive::Static(self)
    }
}

impl IntoMaybeReactive<gtk_dom::layout::FlexWrap>
    for gtk_dom::layout::FlexWrap
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexWrap> {
        MaybeReactive::Static(self)
    }
}

// Closure impls per concrete output type. (We can't write a single
// `impl<T, F> IntoMaybeReactive<T> for F where F: Fn() -> T` because
// it would conflict with the static impls above.)

impl<F> IntoMaybeReactive<String> for F
where
    F: Fn() -> String + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<bool> for F
where
    F: Fn() -> bool + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<bool> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<f64> for F
where
    F: Fn() -> f64 + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<f64> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<f32> for F
where
    F: Fn() -> f32 + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<f32> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::FlexDirection> for F
where
    F: Fn() -> gtk_dom::layout::FlexDirection + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::FlexDirection> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::JustifyContent> for F
where
    F: Fn() -> gtk_dom::layout::JustifyContent + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::JustifyContent> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::AlignItems> for F
where
    F: Fn() -> gtk_dom::layout::AlignItems + Send + 'static,
{
    fn into_maybe_reactive(
        self,
    ) -> MaybeReactive<gtk_dom::layout::AlignItems> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<gtk_dom::layout::FlexWrap> for F
where
    F: Fn() -> gtk_dom::layout::FlexWrap + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<gtk_dom::layout::FlexWrap> {
        MaybeReactive::Reactive(Box::new(self))
    }
}

impl<F> IntoMaybeReactive<usize> for F
where
    F: Fn() -> usize + Send + 'static,
{
    fn into_maybe_reactive(self) -> MaybeReactive<usize> {
        MaybeReactive::Reactive(Box::new(self))
    }
}
