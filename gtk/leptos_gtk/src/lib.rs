//! GTK4 native UI port for leptos-mac.
//!
//! The user-facing crate. End-user examples write
//! `leptos = { package = "leptos_gtk", path = ... }` in their
//! Cargo.toml, then `use leptos::prelude::*` resolves to the
//! gtk-specialized prelude here.
//!
//! The [`Dom`] unit type is this crate's [`renderer::Renderer`] impl.

#![cfg(feature = "gtk")]
#![allow(missing_docs)]

pub mod directive;
pub mod element_gtk;
pub mod event_gtk;
pub mod gtk;
pub mod keys;
pub mod mount;
pub mod renderer_gtk;
pub mod svg_gtk;

pub use renderer_gtk::Dom;

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
// when this crate is brought in as `leptos = { package = "leptos_gtk" }`.
// ---------------------------------------------------------------------

/// Re-export of the renderer-agnostic core (Show, For, IntoView, etc.).
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

    /// Re-export of the gtk builders/window/etc., for the
    /// `::leptos::tachys::gtk::*` paths some examples reference
    /// directly.
    pub mod gtk {
        pub use crate::gtk::*;
    }

    pub mod html {
        pub mod element {
            //! GTK-flavoured element builders (button, vstack, label, ...).
            pub use crate::element_gtk::*;
        }
        pub mod event {
            pub use crate::event_gtk::*;
        }
        pub mod attribute {
            pub use crate::keys::*;
        }
        pub mod directive {
            pub use crate::directive::*;
        }
    }

    pub mod svg {
        pub use crate::svg_gtk::*;
    }

    /// Marker-equivalent for tachys' nightly Static optimization.
    pub mod mathml {}
}

/// GTK-specialized [`IntoView`](leptos::IntoView). Pinning R to
/// [`Dom`] lets user code write `impl IntoView` (the type parameter
/// is resolved at the trait boundary).
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

/// User prelude — items end-user examples bring into scope.
pub mod prelude {
    // Re-export the leptos core prelude FIRST so our gtk-specialized
    // overrides below shadow it (specifically `IntoView`).
    pub use crate::core::prelude::*;

    pub use crate::{IntoAttributeValue, IntoView};

    // Mounting
    pub use crate::mount::{mount_to_window, run};

    // GTK-flavoured element builders, exposed as bare functions so
    // user code that writes them directly (instead of via the
    // `view!{}` macro) just works.
    pub use crate::gtk::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, checkbox, hstack, label, pop_up_button,
            secure_text_field, slider, stack_view, text_field, view,
            vstack,
        },
        node_ref::NodeRef,
        AlignItems, FlexDirection, JustifyContent,
    };

    pub use crate::Dom;
}
