//! Attribute-value plumbing for iOS elements.
//!
//! Builder methods like `.title(...)` accept anything implementing
//! [`IntoMaybeReactive<T>`]. Two impl shapes:
//!
//! - **`T` itself** — wrapped as `MaybeReactive::Static`.
//! - **`F: Fn() -> T`** — wrapped as `MaybeReactive::Reactive`. At
//!   build time the builder registers a [`RenderEffect`] that re-runs
//!   the closure on signal changes and updates the underlying UIView
//!   property each time.
//!
//! [`MaybeReactive`] and the [`install`] driver come from
//! `renderer::attrs` (shared across ports). The [`IntoMaybeReactive`]
//! trait, however, is **port-local**: Rust's orphan rule blocks
//! closure-form impls (`impl<F: Fn() -> Local> renderer::IntoMaybeReactive<Local> for F`)
//! from any crate that doesn't own the trait — `F` is the impl's
//! Self type and appears before the trait's first local type
//! parameter. The local trait shadow dodges this.
//!
//! [`RenderEffect`]: reactive_graph::effect::RenderEffect

pub use leptos_native::renderer::attrs::{install, AlignSelf, Dim, MaybeReactive};
use crate::dom::{
    layout::{AlignItems, FlexDirection, FlexWrap, JustifyContent},
    Color, Date, DatePickerStyle, TextAlignment,
};

/// Conversion trait so attribute setters can take either a bare
/// value or a `Fn() -> T` closure transparently. Port-local — see
/// the module docs for why.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

// `impl_pair!` (the static + closure `IntoMaybeReactive` impl generator)
// is identical across ports → it lives in core. The impls it generates
// resolve against this port's local `IntoMaybeReactive` + `MaybeReactive`.
use leptos_native::impl_pair;

impl_pair!(
    String, bool, i32, f32, f64, usize, Dim,
    FlexDirection, JustifyContent, AlignItems, FlexWrap,
    Color, Date, TextAlignment, DatePickerStyle,
    Option<Vec<u8>>,
);

// Conversion sugars that don't fit the static-T-for-T pattern.

impl IntoMaybeReactive<String> for &str {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self.to_owned())
    }
}

impl IntoMaybeReactive<Dim> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(Dim::Px(self))
    }
}
