//! Cocoa/AppKit native UI port for leptos-mac.
//!
//! The user-facing crate. End-user examples write
//! `leptos = { package = "leptos_cocoa", path = ... }` in their
//! Cargo.toml, then `use leptos::prelude::*` resolves to the
//! cocoa-specialized prelude here.
//!
//! The [`Dom`] unit type is this crate's [`renderer::Renderer`] impl.

#![cfg(target_os = "macos")]
#![allow(missing_docs)]

pub mod cocoa;
pub mod directive;
pub mod element_macos;
pub mod event_macos;
pub mod keys;
pub mod mount;
pub mod renderer_cocoa;
pub mod svg_macos;

pub use renderer_cocoa::Dom;

/// Bind/attribute keys re-exported under the `leptos::attr` path the
/// `bind:foo=value` macro syntax expands to (`::leptos::attr::Value`,
/// `::leptos::attr::Checked`).
pub mod attr {
    pub use crate::keys::*;
}

// ---------------------------------------------------------------------
// User-facing surface
//
// Exposed under the names the leptos_macro view!{} expansion expects
// (`::leptos::tachys::html::element::*`, `::leptos::prelude::*`, etc.)
// when this crate is brought in as `leptos = { package = "leptos_cocoa" }`.
// ---------------------------------------------------------------------

/// Re-export of the renderer-agnostic core (Show, For, IntoView, etc.).
/// Examples that want a specific item not in the prelude can reach for
/// it via `leptos::core::...` (e.g. `core::IntoView`).
pub use leptos as core;

// Re-exports the `#[component]` macro emits paths into.
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

/// View-tree machinery + element builders + events under the path
/// shape the `view!{}` macro emits (`::leptos::tachys::html::element::*`,
/// `::leptos::tachys::view::*`, `::leptos::tachys::svg::*`).
pub mod tachys {
    pub use ::renderer::view;

    /// Re-export of the cocoa builders/window/etc., for the
    /// `::leptos::tachys::cocoa::*` paths some examples reference
    /// directly (e.g. block_layout uses `tachys::cocoa::FlexDirection`).
    pub mod cocoa {
        pub use crate::cocoa::*;
    }

    pub mod html {
        pub mod element {
            //! Cocoa-flavoured element builders (button, vstack, label, ...).
            pub use crate::element_macos::*;
        }
        pub mod event {
            pub use crate::event_macos::*;
        }
        pub mod attribute {
            pub use crate::keys::*;
        }
        pub mod directive {
            pub use crate::directive::*;
        }
    }

    pub mod svg {
        pub use crate::svg_macos::*;
    }

    /// Marker-equivalent for tachys' nightly Static optimization. The
    /// stable macro path doesn't emit this; defined as an empty module
    /// so any stray reference path-resolves.
    pub mod mathml {}
}

/// Cocoa-specialized [`IntoView`](leptos::IntoView). Pinning R to
/// [`Dom`] lets user code write `impl IntoView` (the type parameter
/// is resolved at the trait boundary) without sprinkling `<Dom>` —
/// or worse, `<R: Renderer>` — through every component signature.
pub trait IntoView: leptos::IntoView<Dom> {}
impl<T: leptos::IntoView<Dom>> IntoView for T {}

/// Identity trait the leptos_macro view!{} expansion emits as
/// `::leptos::prelude::IntoAttributeValue::into_attribute_value(...)`
/// around attribute values. Upstream this normalised values into a
/// SSR-friendly `AttributeValue` shape; on native the value is
/// already the right type so the trait is a no-op identity.
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

/// User prelude — the items end-user examples bring into scope.
pub mod prelude {
    // Re-export the leptos core prelude FIRST so our cocoa-specialized
    // overrides below shadow it (specifically `IntoView`).
    pub use crate::core::prelude::*;

    // Cocoa-specialized IntoView (pinned to Dom; no R param).
    pub use crate::{IntoAttributeValue, IntoView};

    // Mounting
    pub use crate::mount::{mount_to_window, run};

    // Cocoa-flavoured element builders, exposed as bare functions
    // (`button()`, `vstack()`, etc.) so user code that writes them
    // directly (instead of via the `view!{}` macro) just works.
    pub use crate::cocoa::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, hstack, label, stack_view, text_field, view, vstack,
            // Shadow the renderer-common `WithText` (re-exported via
            // `crate::core::prelude::*` above) with the port-local
            // one. The renderer trait is generic over `<C, A>` and
            // uses renderer-common's `IntoMaybeReactive`, which has no
            // impls for `Color` / `NSTextAlignment`. The port-local
            // trait pins those types and uses our local
            // `IntoMaybeReactive` (cocoa/attr.rs). Examples using
            // `font_size=...`, `alignment=...`, `text_color=...` need
            // this in scope.
            WithText,
        },
        node_ref::NodeRef,
        AlignItems, FlexDirection, JustifyContent,
    };

    // cocoa_dom helpers commonly used by examples (timers, persistent
    // storage, native colour types, key events).
    pub use cocoa_dom::{
        local_storage, set_interval, set_interval_with_handle, Color,
        IntervalError, IntervalHandle, KeyEvent, Storage, StorageError,
    };
    pub use crate::Dom;
}
