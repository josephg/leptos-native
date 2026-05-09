//! Spread / `add_any_attr` machinery for native targets.
//!
//! The leptos_macro `view!{}` expansion emits `.add_any_attr((spread1,
//! spread2, …))` on a component's return value when the user writes
//! `<MyComponent on:click=… use:foo=…>`. Each "spread" is an attribute
//! that needs to install itself on whatever underlying element ends up
//! in the tree. The trait machinery here is how those attributes
//! trickle down through wrapper types ([`View`], [`OwnedView`], …)
//! and land on a leaf builder.
//!
//! Phase-9 minimal port of upstream `tachys::view::add_attr::AddAnyAttr`.
//! Differences:
//!
//! - Single method, no `Output<NewAttr>` type-changing accumulation.
//!   Native installs at runtime via a `pending_spreads` Vec on the
//!   leaf builder; no need for the attribute to become part of the
//!   static type.
//! - No `Attribute` trait with `to_html` / `hydrate` / `keys` /
//!   `dry_resolve` / `into_owned` / etc. — that's the SSR machinery
//!   we deleted in Phase 8.
//! - No `IntoAnyAttribute` / `AnyAttribute` enum — only
//!   [`OnAttribute`](crate::view::add_any_attr::ApplyAttr) and a few
//!   handful of native attribute kinds need to round-trip.
//! - Branching wrappers ([`Either`], `Option<T>`, `Vec<T>`,
//!   reactive closures) are explicitly *not* supported here —
//!   `<Show on:click=…>` is deferred until we figure out the
//!   re-attach-on-rebuild semantics.

use crate::renderer::Renderer;

/// Receive deferred attribute spreads. Implemented on:
/// - leaf builders (Button, Label, …) — push to their internal
///   pending-spreads Vec, drain in `Render::build`.
/// - wrapper types ([`View<T>`], [`OwnedView<T>`]) — forward to the
///   wrapped value's `AddAnyAttr<R>`.
pub trait AddAnyAttr<R: Renderer>: Sized {
    /// Stash `attr` so it gets applied to this view's eventual
    /// element. Returns `Self` (no static type-change — see module
    /// docs).
    fn add_any_attr<A: ApplyAttr<R>>(self, attr: A) -> Self;
}

/// An attribute that knows how to install itself on a built
/// platform element. Each platform's `OnAttribute` (and any future
/// directive / use-attr / etc.) implements `ApplyAttr<Dom>` for
/// its respective `Dom`.
pub trait ApplyAttr<R: Renderer>: Send + 'static {
    /// Move-attach this attribute to a built element. Called
    /// during `Render::build` on the leaf builder, after the
    /// underlying NSView/UIView has been constructed and the
    /// builder's normal handlers installed.
    fn apply_to(self, el: &R::Element);
}

// ---------------------------------------------------------------------
// Tuple impls of ApplyAttr — `.add_any_attr((a,))`, `.add_any_attr((a, b))`, …
//
// The macro always emits a tuple (`(#(#spreads,)*)`), even for a single
// attribute (`(SoleAttr,)`). The 0-tuple `()` case never reaches
// `add_any_attr` because the macro skips emission when the spread
// list is empty.
// ---------------------------------------------------------------------

impl<R: Renderer> ApplyAttr<R> for () {
    fn apply_to(self, _el: &R::Element) {}
}

macro_rules! impl_apply_attr_tuple {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<R: Renderer, $($T),+> ApplyAttr<R> for ($($T,)+)
        where
            $($T: ApplyAttr<R>,)+
        {
            fn apply_to(self, el: &R::Element) {
                $( self.$idx.apply_to(el); )+
            }
        }
    };
}

impl_apply_attr_tuple!((0, A));
impl_apply_attr_tuple!((0, A), (1, B));
impl_apply_attr_tuple!((0, A), (1, B), (2, C));
impl_apply_attr_tuple!((0, A), (1, B), (2, C), (3, D));
impl_apply_attr_tuple!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_apply_attr_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_apply_attr_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
impl_apply_attr_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));

// ---------------------------------------------------------------------
// No-op fallback impls of `AddAnyAttr<R>` for the terminal / dynamic
// view types. The attribute is silently dropped for these; the user's
// `<Component on:click=…>` only does anything if the component
// returns a tree whose root is a leaf builder (Button, Label, …) or
// a wrapper that forwards (View<T>, OwnedView<T>).
//
// Branching wrappers (`Either`, `Option<T>`, `Vec<T>`, reactive
// closures) get this no-op too — supporting them properly needs
// re-attach-on-rebuild semantics, which is deferred.
// ---------------------------------------------------------------------

impl<R: Renderer> AddAnyAttr<R> for () {
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {}
}

macro_rules! noop_add_any_attr {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl<R: Renderer> AddAnyAttr<R> for $ty {
                fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
                    self
                }
            }
        )+
    };
}

noop_add_any_attr!(
    String,
    &'static str,
    bool,
    char,
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
    f32, f64,
    std::borrow::Cow<'static, str>,
    std::rc::Rc<str>,
    std::sync::Arc<str>,
);

// Either — branching wrapper, defer.
impl<R: Renderer, A, B> AddAnyAttr<R> for either_of::Either<A, B> {
    fn add_any_attr<Attr: ApplyAttr<R>>(self, _attr: Attr) -> Self {
        self
    }
}

// Option — branching, defer.
impl<R: Renderer, T> AddAnyAttr<R> for Option<T> {
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        self
    }
}

// Vec — multi-child, defer.
impl<R: Renderer, T> AddAnyAttr<R> for Vec<T> {
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        self
    }
}

// Result<T, E> — error-boundary branching, defer.
impl<R: Renderer, T, E> AddAnyAttr<R> for Result<T, E> {
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        self
    }
}

// Tuples — multi-child, defer (we'd need to pick which child gets
// the attribute).
macro_rules! impl_addanyattr_tuple_noop {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<R: Renderer, $($T),+> AddAnyAttr<R> for ($($T,)+) {
            fn add_any_attr<__A: ApplyAttr<R>>(self, _attr: __A) -> Self {
                self
            }
        }
    };
}

impl_addanyattr_tuple_noop!((0, A));
impl_addanyattr_tuple_noop!((0, A), (1, B));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C), (3, D));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
impl_addanyattr_tuple_noop!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
