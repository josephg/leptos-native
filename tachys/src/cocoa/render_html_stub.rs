//! Stub `RenderHtml` (and optionally `AddAnyAttr`) impls for the
//! Cocoa element types so they satisfy the `IntoView` blanket impl
//! in `leptos::into_view`.
//!
//! Why this exists: leptos's `IntoView` trait is bounded
//! `Render + RenderHtml + Send`. We have real `Render` impls (in
//! [`crate::cocoa::element`]); the SSR/hydration story is stubbed
//! since neither concept is real on native macOS. See
//! `implementation_log.md` (the "stub `hydrate` to delegate to
//! `build`" entry) for the cleanup story.
//!
//! `cocoa_stub_view_impls!($ty)` stubs BOTH `AddAnyAttr` (drops
//! attrs) and `RenderHtml`. Use for builders not yet refactored to
//! the type-parametric attribute pipeline. Builders that *have*
//! been refactored (`<At = ()>` form) get their `RenderHtml` impl
//! emitted by `impl_typed_attrs_for!` in `element.rs` instead.
//!
//! Each stub:
//!   - `to_html_with_buf`: empty (no SSR — write nothing).
//!   - `hydrate`: delegates to `Render::build` (no real hydration —
//!     create from scratch).
//!   - `dry_resolve` / `resolve`: passthrough (no async data
//!     resolution involved in our types).
//!   - `into_owned`: returns self (each type is already 'static if
//!     its captured closures are 'static, which they must be to be
//!     stored).
//!   - `MIN_LENGTH`: 0 — only used for SSR string-buffer sizing.

/// Combined stub: AddAnyAttr (drops attrs) + RenderHtml. Used
/// for builders not yet refactored to the typed-attribute path.
///
/// Currently unused — every cocoa builder goes through
/// `impl_typed_attrs_for!` now. Keeping for future builders that
/// don't fit the typed-attrs pattern.
#[allow(unused_macros)]
macro_rules! cocoa_stub_view_impls {
    ($ty:ty) => {
        impl $crate::view::add_attr::AddAnyAttr for $ty {
            type Output<NewAttr: $crate::html::attribute::Attribute> = $ty;

            fn add_any_attr<NewAttr: $crate::html::attribute::Attribute>(
                self,
                _attr: NewAttr,
            ) -> Self::Output<NewAttr> {
                self
            }
        }

        impl $crate::view::RenderHtml for $ty {
            type AsyncOutput = Self;
            type Owned = Self;

            const MIN_LENGTH: usize = 0;

            fn dry_resolve(&mut self) {}

            async fn resolve(self) -> Self::AsyncOutput {
                self
            }

            fn to_html_with_buf(
                self,
                _buf: &mut String,
                _position: &mut $crate::view::Position,
                _escape: bool,
                _mark_branches: bool,
                _extra_attrs: Vec<
                    $crate::html::attribute::any_attribute::AnyAttribute,
                >,
            ) {
            }

            fn hydrate<const FROM_SERVER: bool>(
                self,
                _cursor: &$crate::hydration::Cursor,
                _position: &$crate::view::PositionState,
            ) -> Self::State {
                <Self as $crate::view::Render>::build(self)
            }

            fn into_owned(self) -> Self::Owned {
                self
            }
        }
    };
}

#[allow(unused_imports)]
pub(super) use cocoa_stub_view_impls;
