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
//! Minimal port of upstream `tachys::view::add_attr::AddAnyAttr`.
//! Differences from upstream:
//!
//! - Single method, no `Output<NewAttr>` type-changing accumulation.
//!   Native installs at runtime via a `pending_spreads` Vec on the
//!   leaf builder; no need for the attribute to become part of the
//!   static type.
//! - No `Attribute` trait (the SSR machinery — `to_html` / `hydrate`
//!   / `keys` / `dry_resolve` / `into_owned`).
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
// Fallback impls of `AddAnyAttr<R>` for the terminal / dynamic view
// types. These cases are required by the type system (IntoView<R>
// has AddAnyAttr<R> as a supertrait, so every renderable type must
// implement it) but have no sensible install target. They PANIC at
// build time rather than silently swallowing the attribute, so users
// see a clear failure when they write `<Component on:click=…>` and
// the component returns one of these shapes.
//
// All these panics are caught at view *construction* time (when
// `add_any_attr` is called during the macro-emitted builder chain),
// well before the AppKit run loop starts — the failure is loud and
// immediate, not a silent UI bug.
// ---------------------------------------------------------------------

#[track_caller]
fn panic_terminal(kind: &str) -> ! {
    panic!(
        "AddAnyAttr<R>::add_any_attr called on a {kind} view. Spread \
         attributes (e.g. `<Component on:click=…>`) require a component \
         that returns a builder element (button, label, vstack, …) — \
         not a plain text or numeric value."
    );
}

#[track_caller]
fn panic_branching(kind: &str) -> ! {
    panic!(
        "AddAnyAttr<R>::add_any_attr called on a {kind} view (a \
         branching/reactive wrapper). This isn't supported yet — it \
         needs re-attach-on-rebuild semantics that haven't been \
         designed. Workaround: attach the attribute to the inner \
         element directly. e.g. instead of `<Show on:click=h \
         when=…>{{…}}</Show>`, write `<Show when=…><view \
         on:click=h>…</view></Show>`."
    );
}

impl<R: Renderer> AddAnyAttr<R> for () {
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic_terminal("`()` (empty)")
    }
}

macro_rules! terminal_add_any_attr {
    ($($ty:ty => $name:expr),+ $(,)?) => {
        $(
            impl<R: Renderer> AddAnyAttr<R> for $ty {
                #[track_caller]
                fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
                    panic_terminal($name)
                }
            }
        )+
    };
}

terminal_add_any_attr!(
    String => "String",
    &'static str => "&str",
    bool => "bool",
    char => "char",
    i8 => "i8", i16 => "i16", i32 => "i32",
    i64 => "i64", i128 => "i128", isize => "isize",
    u8 => "u8", u16 => "u16", u32 => "u32",
    u64 => "u64", u128 => "u128", usize => "usize",
    f32 => "f32", f64 => "f64",
    std::borrow::Cow<'static, str> => "Cow<str>",
    std::rc::Rc<str> => "Rc<str>",
    std::sync::Arc<str> => "Arc<str>",
);

impl<R: Renderer, A, B> AddAnyAttr<R> for either_of::Either<A, B> {
    #[track_caller]
    fn add_any_attr<Attr: ApplyAttr<R>>(self, _attr: Attr) -> Self {
        panic_branching("`Either<A, B>`")
    }
}

impl<R: Renderer, T> AddAnyAttr<R> for Option<T> {
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic_branching("`Option<T>`")
    }
}

impl<R: Renderer, T> AddAnyAttr<R> for Vec<T> {
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic_branching("`Vec<T>` (`<For>` body)")
    }
}

impl<R: Renderer, T, E> AddAnyAttr<R> for Result<T, E> {
    #[track_caller]
    fn add_any_attr<A: ApplyAttr<R>>(self, _attr: A) -> Self {
        panic_branching("`Result<T, E>` (ErrorBoundary body)")
    }
}

// Tuples — multi-child views. Could route to a specific child but
// it's ambiguous; surface the ambiguity to the user.
macro_rules! impl_addanyattr_tuple_panic {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<R: Renderer, $($T),+> AddAnyAttr<R> for ($($T,)+) {
            #[track_caller]
            fn add_any_attr<__A: ApplyAttr<R>>(self, _attr: __A) -> Self {
                panic!(
                    "AddAnyAttr<R>::add_any_attr called on a tuple of \
                     views. Tuples have multiple top-level children — \
                     it's ambiguous which one should receive the spread \
                     attribute. Wrap the desired child in its own \
                     component or apply the attribute to a specific \
                     element instead."
                )
            }
        }
    };
}

impl_addanyattr_tuple_panic!((0, A));
impl_addanyattr_tuple_panic!((0, A), (1, B));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C), (3, D));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
impl_addanyattr_tuple_panic!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
