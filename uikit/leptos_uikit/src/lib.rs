//! UIKit native UI port for leptos-mac (iOS).
//!
//! The user-facing crate. End-user examples write
//! `leptos = { package = "leptos_uikit", path = ... }` in their
//! Cargo.toml, then `use leptos::prelude::*` resolves to the
//! UIKit-specialized prelude here.
//!
//! Mirror of `cocoa/leptos_cocoa/src/lib.rs`; see commentary there.

#![cfg(target_os = "ios")]
#![allow(missing_docs)]

pub mod directive;
pub mod element_ios;
pub mod event_ios;
pub mod ios;
pub mod keys;
pub mod mount;
pub mod renderer_ios;
pub mod svg_ios;

/// Aliased path for `leptos::mount_ios::run` so existing iOS example
/// code (`leptos::mount_ios::run(...)`) keeps compiling under the
/// `leptos = { package = "leptos_uikit", ... }` shape.
pub mod mount_ios {
    pub use crate::mount::*;
}

pub use renderer_ios::Dom;

pub mod attr {
    pub use crate::keys::*;
}

pub use leptos as core;

pub use leptos::component;
pub use leptos::reactive;
pub use leptos::__reexports;
pub use leptos::children;
pub use leptos::context;
#[doc(hidden)]
pub use leptos::typed_builder;
#[doc(hidden)]
pub use leptos::typed_builder_macro;
pub use leptos::callback;

pub mod tachys {
    pub use ::renderer::view;

    /// Re-export of the iOS builders + helpers, for the
    /// `::leptos::tachys::ios::*` paths some user code references
    /// directly.
    pub mod ios {
        pub use crate::ios::*;
    }

    pub mod html {
        pub mod element {
            pub use crate::element_ios::*;
        }
        pub mod event {
            pub use crate::event_ios::*;
        }
        pub mod attribute {
            pub use crate::keys::*;
        }
        pub mod directive {
            pub use crate::directive::*;
        }
    }

    pub mod svg {
        pub use crate::svg_ios::*;
    }

    pub mod mathml {}
}

/// UIKit-specialized [`IntoView`](leptos::IntoView). Pins R to [`Dom`]
/// so user code writes `impl IntoView` (no R param visible).
pub trait IntoView: leptos::IntoView<Dom> {}
impl<T: leptos::IntoView<Dom>> IntoView for T {}

/// Identity trait the leptos_macro emits as
/// `::leptos::prelude::IntoAttributeValue::into_attribute_value(...)`
/// around attribute values. No-op identity on native.
pub trait IntoAttributeValue {
    type Output;
    fn into_attribute_value(self) -> Self::Output;
}

impl<T> IntoAttributeValue for T {
    type Output = T;
    fn into_attribute_value(self) -> Self {
        self
    }
}

pub mod prelude {
    // Re-export the leptos core prelude FIRST so our UIKit-specialized
    // overrides below shadow it (specifically `IntoView`).
    pub use crate::core::prelude::*;

    pub use crate::{IntoAttributeValue, IntoView};

    pub use crate::mount::run;

    pub use crate::ios::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, hstack, label, scroll_view, secure_text_field,
            slider, switch_, text_field, vstack, WithText,
        },
        node_ref::NodeRef,
        FlexDirection, JustifyContent,
    };

    // Renderer-common attribute-accessor traits that builders impl.
    // Importing the traits brings the chainable setters
    // (`.padding(...)`, `.alpha(...)`, `.flex_grow(...)`, ...) into
    // scope on every builder.
    pub use ::renderer::attrs::{
        AlignSelf, Dim, WithLayout, WithUniversal,
    };

    pub use ios_dom::{
        local_storage, set_interval, set_interval_with_handle, Color,
        IntervalError, IntervalHandle, KeyEvent, Storage, StorageError,
    };

    pub use crate::Dom;
}
