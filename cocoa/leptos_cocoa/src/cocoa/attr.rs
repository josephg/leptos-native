//! Attribute-value plumbing for Cocoa elements.
//!
//! Builder methods like `.title(...)` accept anything implementing
//! [`IntoMaybeReactive<T>`]. Two impl shapes:
//!
//! - **`T` itself** — wrapped as `MaybeReactive::Static`.
//! - **`F: Fn() -> T`** — wrapped as `MaybeReactive::Reactive`. At
//!   build time the builder registers a [`RenderEffect`] that re-runs
//!   the closure on signal changes and updates the underlying NSView
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
//! The `RenderEffect` returned by `install` is owned by the
//! element's `State`, so it lives exactly as long as the element
//! is mounted.

pub use renderer::attrs::{install, AlignSelf, Dim, Edges, MaybeReactive};
use cocoa_dom::{
    layout::{AlignItems, FlexDirection, FlexWrap, JustifyContent},
    toolbar::{ToolbarDisplayMode, WindowToolbarStyle},
    Color, Date, DatePickerStyle, Icon, LineBreak, SegmentStyle, TextAlignment,
};

/// Conversion trait so attribute setters can take either a bare
/// value or a `Fn() -> T` closure transparently. Port-local — see
/// the module docs for why.
pub trait IntoMaybeReactive<T: 'static> {
    fn into_maybe_reactive(self) -> MaybeReactive<T>;
}

/// Generate the static + closure impls of [`IntoMaybeReactive<T>`]
/// for one or more concrete `T`s. The two impls are the same shape
/// for every type — keeping them in a macro avoids re-stating ~13
/// lines of `MaybeReactive::Static(self)` / `Reactive(Box::new(self))`
/// per impl pair. Macro is invoked immediately below, so every impl
/// it generates is type-checked at compile time.
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
    String, bool, i32, f32, f64, usize, Dim, Edges,
    FlexDirection, JustifyContent, AlignItems, FlexWrap,
    Color, Date, TextAlignment, LineBreak, SegmentStyle, DatePickerStyle,
    ToolbarDisplayMode, WindowToolbarStyle, Icon,
);

// Conversion sugars that don't fit the static-T-for-T pattern.

/// `&str` → `MaybeReactive<String>` so callers can pass `"hi"`
/// without `.to_string()`.
impl IntoMaybeReactive<String> for &str {
    fn into_maybe_reactive(self) -> MaybeReactive<String> {
        MaybeReactive::Static(self.to_owned())
    }
}

/// `f32` → `MaybeReactive<Dim>` so callers can pass `width(120.0)`
/// without `Dim::px(...)`.
impl IntoMaybeReactive<Dim> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Dim> {
        MaybeReactive::Static(Dim::Px(self))
    }
}

/// `f32` → `Edges::all(...)` so `padding=8.0` keeps working as
/// shorthand for uniform padding even though the field type is
/// `Edges`. This impl is duplicated from `renderer::attrs` because
/// the orphan rule blocks closure-form `IntoMaybeReactive` impls
/// from existing in the renderer crate (the trait can't be foreign
/// to *both* the impl and the type) — see the `attr.rs` module doc.
impl IntoMaybeReactive<Edges> for f32 {
    fn into_maybe_reactive(self) -> MaybeReactive<Edges> {
        MaybeReactive::Static(Edges::all(self))
    }
}
