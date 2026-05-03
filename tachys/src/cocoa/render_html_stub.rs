//! Stub `RenderHtml` + `AddAnyAttr` impls for the Cocoa element
//! types so they satisfy the `IntoView` blanket impl in
//! `leptos::into_view`.
//!
//! Why this exists: leptos's `IntoView` trait is bounded
//! `Render + RenderHtml + Send`. We have real `Render` impls (in
//! [`crate::cocoa::element`]); the rest need to exist *for the type
//! checker*, even though SSR / hydration are not real concepts on
//! native macOS. See `implementation_log.md` (the "stub `hydrate` to
//! delegate to `build`" entry) for the cleanup story.
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

/// Macro to install the stub impls on a non-generic Cocoa element type
/// (Button, Label, TextField). `Self::Owned = Self` requires Self to
/// be 'static, which non-generic Cocoa types are.
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
                // No SSR on native — emit nothing.
            }

            fn hydrate<const FROM_SERVER: bool>(
                self,
                _cursor: &$crate::hydration::Cursor,
                _position: &$crate::view::PositionState,
            ) -> Self::State {
                // Hydration is stubbed on native (see implementation_log
                // 2026-05-03 entry). Fall back to a fresh build —
                // semantically correct for the only path that matters
                // (mount_to_window, which doesn't hydrate).
                <Self as $crate::view::Render>::build(self)
            }

            fn into_owned(self) -> Self::Owned {
                self
            }
        }
    };
}

pub(super) use cocoa_stub_view_impls;
