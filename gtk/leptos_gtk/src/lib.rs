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
/// `::leptos::tachys::view::*`).
///
/// The web Leptos macro additionally routed SVG-tag-named elements
/// through `tachys::svg::*` and emitted `.attr(name, value)` for
/// every attribute. On native there's no SVG renderer and no
/// untyped `.attr()` slot, so this fork's macro routes every tag
/// through `tachys::html::element::*`.
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
}

/// GTK-specialized [`IntoView`](leptos::IntoView). Pinning R to
/// [`Dom`] lets user code write `impl IntoView` (the type parameter
/// is resolved at the trait boundary).
pub trait IntoView: leptos::IntoView<Dom> {}
impl<T: leptos::IntoView<Dom>> IntoView for T {}

/// User prelude — items end-user examples bring into scope.
pub mod prelude {
    // Re-export the leptos core prelude FIRST so our gtk-specialized
    // overrides below shadow it (specifically `IntoView`).
    // (`IntoAttributeValue` lives in `common/leptos` and comes in via
    // `core::prelude::*`.)
    pub use crate::core::prelude::*;

    pub use crate::IntoView;

    // Mounting
    pub use crate::mount::{mount_to_window, run};

    // GTK-flavoured element builders, exposed as bare functions so
    // user code that writes them directly (instead of via the
    // `view!{}` macro) just works.
    pub use crate::gtk::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, checkbox, grid, hstack, label, pop_up_button,
            secure_text_field, slider, stack_view, text_field, view,
            vstack,
        },
        node_ref::NodeRef,
        AlignContent, AlignItems, FlexDirection, GridAutoFlow,
        GridTemplateComponent, JustifyContent, JustifyItems,
        TrackSizingFunction,
    };

    // Menu builders + portable Modifiers. `<menu_bar>` is a
    // top-level sibling of `<window>` in `run()`; nested `<menu>`s
    // and `<menu_item>`s describe the contents.
    pub use crate::gtk::menu::{
        menu, menu_bar, menu_item, menu_separator, Menu, MenuBar, MenuItem,
        MenuSeparator,
    };
    pub use renderer::menu::Modifiers;

    // Explicit window builder — needed when `run()` (not
    // `mount_to_window`) is used to compose a `<menu_bar>` + window
    // tuple at the top level.
    pub use crate::gtk::window::{window, Window};

    pub use renderer::{
        auto, fit_content, fr, length, max_content, min_content, minmax,
        percent, repeat,
    };
    pub use renderer::attrs::{auto_line, span, GridLine};

    pub use crate::Dom;
}
