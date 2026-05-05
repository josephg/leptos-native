//! Stub `RenderHtml` (and optionally `AddAnyAttr`) impls for the
//! iOS element types so they satisfy the `IntoView` blanket impl.
//! Port from `tachys::cocoa::render_html_stub`.

/// Combined stub: AddAnyAttr (drops attrs) + RenderHtml. Used
/// for builders not yet refactored to the typed-attribute path.
#[allow(unused_macros)]
macro_rules! ios_stub_view_impls {
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
pub(super) use ios_stub_view_impls;
