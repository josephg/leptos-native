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
use ios_dom::{
    layout::{AlignItems, FlexDirection, FlexWrap, JustifyContent},
    Color, Date, DatePickerStyle, TextAlignment,
};

/// Conversion trait so attribute setters can take either a bare
/// value or a `Fn() -> T` closure transparently. Port-local — see
/// the module docs for why.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

/// Generate the static + closure impls of [`IntoMaybeReactive<T>`]
/// for one or more concrete `T`s. See the cocoa equivalent for the
/// rationale.
macro_rules! impl_pair {
    ($($t:ty),* $(,)?) => {
        $(
            impl IntoMaybeReactive<$t> for $t {
                fn into_maybe_reactive(self) -> MaybeReactive<$t> {
                    MaybeReactive::Static(self)
                }
            }
            impl<F> IntoMaybeReactive<$t> for F
            where
                F: Fn() -> $t + Send + 'static,
            {
                fn into_maybe_reactive(self) -> MaybeReactive<$t> {
                    MaybeReactive::Reactive(Box::new(self))
                }
            }
        )*
    };
}

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
