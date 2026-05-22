//! UIKit native UI port for leptos-mac (iOS).
//!
//! The user-facing crate. End-user examples write
//! `leptos = { package = "leptos_uikit", path = ... }` in their
//! Cargo.toml, then `use leptos_native::prelude::*` resolves to the
//! UIKit-specialized prelude here.
//!
//! Mirror of `cocoa/leptos_cocoa/src/lib.rs`; see commentary there.

#![cfg(target_os = "ios")]
#![allow(missing_docs)]

pub mod element_ios;
pub mod event_ios;
pub mod ios;
pub mod keys;
pub mod mount;
pub mod renderer_ios;
pub mod dom;

/// Aliased path for `leptos_native::mount_ios::run` so existing iOS example
/// code (`leptos_native::mount_ios::run(...)`) keeps compiling under the
/// `leptos = { package = "leptos_uikit", ... }` shape.
pub mod mount_ios {
    pub use crate::mount::*;
}

pub use renderer_ios::Dom;

/// iOS-pinned `AnyView` — alias of `renderer::view::AnyView<Dom>`.
pub type AnyView = leptos_native::renderer::view::AnyView<Dom>;

/// iOS-pinned alias of [`leptos_native::children::ChildrenFn`].
pub type ChildrenFn = ::leptos_native::children::ChildrenFn<Dom>;

pub mod attr {
    pub use crate::keys::*;
}

pub use leptos_native as core;

pub use leptos_native::component;
pub use leptos_native::reactive;
pub use leptos_native::__reexports;
pub use leptos_native::children;
pub use leptos_native::context;
#[doc(hidden)]
pub use leptos_native::typed_builder;
#[doc(hidden)]
pub use leptos_native::typed_builder_macro;
pub use leptos_native::callback;

pub mod tachys {
    pub use leptos_native::renderer::view;

    /// Re-export of the iOS builders + helpers, for the
    /// `::leptos_native::tachys::ios::*` paths some user code references
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
        // pub mod directive {
        //     pub use crate::directive::*;
        // }
    }
}

/// UIKit-specialized [`IntoView`](leptos_native::IntoView). Pins R to [`Dom`]
/// so user code writes `impl IntoView` (no R param visible).
pub trait IntoView: leptos_native::IntoView<Dom> {}
impl<T: leptos_native::IntoView<Dom>> IntoView for T {}

pub mod prelude {
    // Re-export the leptos core prelude FIRST so our UIKit-specialized
    // overrides below shadow it (specifically `IntoView`).
    // (`IntoAttributeValue` lives in `common/leptos` and comes in via
    // `core::prelude::*`.)
    pub use crate::core::prelude::*;

    pub use crate::IntoView;

    // Type-erased view container — see crate-level `AnyView` alias.
    pub use crate::AnyView;
    pub use crate::ChildrenFn;
    pub use leptos_native::renderer::view::IntoAny;

    pub use crate::mount::{mount, run};

    pub use crate::ios::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, grid, hstack, label, scroll_view, secure_text_field,
            slider, switch_, text_field, vstack, WithText,
        },
        AlignContent, AlignItems, FlexDirection, GridAutoFlow,
        GridTemplateComponent, JustifyContent, JustifyItems,
        TrackSizingFunction,
    };

    // Grid track-sizing helpers — re-exported from Taffy via
    // renderer so example code can write `[fr(1.0), auto()]`.
    pub use leptos_native::renderer::{
        auto, fit_content, fr, length, max_content, min_content, minmax,
        percent, repeat,
    };
    pub use leptos_native::renderer::attrs::{auto_line, span, GridLine};

    // Renderer-common attribute-accessor traits that builders impl.
    // Importing the traits brings the chainable setters
    // (`.padding(...)`, `.alpha(...)`, `.flex_grow(...)`, ...) into
    // scope on every builder.
    pub use leptos_native::renderer::attrs::{
        AlignSelf, Dim, Overflow, WithLayout, WithUniversal,
    };

    pub use crate::dom::{
        self,
        local_storage, set_interval, set_interval_with_handle, Color,
        IntervalError, IntervalHandle, KeyEvent, Storage, StorageError,
    };
}
