//! GTK4 native UI port for leptos-mac.
//!
//! The user-facing crate. End-user examples write
//! `leptos = { package = "leptos_gtk", path = ... }` in their
//! Cargo.toml, then `use leptos_native::prelude::*` resolves to the
//! gtk-specialized prelude here.
//!
//! The [`GtkDom`] unit type is this crate's [`renderer::Renderer`] impl.

#![cfg(feature = "gtk")]
#![allow(missing_docs)]

// `leptos_platform` is the sentinel name the `leptos_macro` proc-macros
// emit absolute paths against. Self-aliased here so that the
// `#[component]` / `view!{}` macros invoked inside this crate resolve
// cleanly.
extern crate self as leptos_platform;

pub mod event_gtk;
pub mod gtk;
pub mod mount;
pub mod renderer_gtk;

pub use renderer_gtk::GtkDom;

pub mod dom;
#[cfg(feature = "devtools")]
pub mod devtools;

// Reexport these so 3rd party code can use our version of them.
pub use {gio, glib, gtk4};


/// GTK-pinned `AnyView` — alias of `renderer::view::AnyView<Dom>`.
pub type AnyView = renderer::view::AnyView<GtkDom>;

/// GTK-pinned alias of [`leptos_native::children::ChildrenFn`].
pub type ChildrenFn = children::ChildrenFn<GtkDom>;

/// Bind/attribute keys re-exported under the `leptos_native::attr` path the
/// `bind:foo=value` macro syntax expands to (`::leptos_native::attr::Value`,
/// `::leptos_native::attr::Checked`).
pub mod attr {
    pub use leptos_native::renderer::attr_keys::{AttributeKey, Checked, Value};
}

// ---------------------------------------------------------------------
// User-facing surface
//
// Exposed under the names the leptos_macro view!{} expansion expects
// (`::leptos_native::tachys::html::element::*`, `::leptos_native::prelude::*`, etc.)
// when this crate is brought in as `leptos = { package = "leptos_gtk" }`.
// ---------------------------------------------------------------------

/// Re-export of the renderer-agnostic core (Show, For, IntoView, etc.).
pub use leptos_native as core;

// Re-exports the `#[component]` macro emits paths into.
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
use leptos_native::renderer;

/// View-tree machinery + element builders + events under the path
/// shape the `view!{}` macro emits (`::leptos_native::tachys::html::element::*`,
/// `::leptos_native::tachys::view::*`).
///
// Flat modules at the paths the `view!{}` macro emits:
// `::leptos_platform::element::*`, `::leptos_platform::event::*`,
// `::leptos_platform::attribute::*`, `::leptos_platform::view::*`,
// `::leptos_platform::reactive_graph::bind::*`.
//
// (Upstream had a `tachys::html::*` namespace; we flattened it here.)

/// GTK-flavoured element builders (button, vstack, label, ...).
pub mod element {
    pub use crate::gtk::element::{
        button, checkbox, grid, hstack, label, pop_up_button, secure_text_field,
        slider, stack, stack_view, text_field, toggle, vstack,
    };

    // Menus: `<menu_bar>` + `<menu>` + `<menu_item>` + `<menu_separator>`.
    // `<menu_bar>` sits as a top-level sibling of `<window>` in `run()`;
    // the others only make sense nested inside.
    pub use crate::gtk::menu::{menu, menu_bar, menu_item, menu_separator};
}

/// GTK-flavoured event descriptors.
pub mod event {
    pub use crate::event_gtk::*;
}

/// Attribute / bind-key markers (Value, Checked, ...).
pub mod attribute {
    pub use leptos_native::renderer::attr_keys::{AttributeKey, Checked, Value};
}

/// Renderer view machinery (View, iterators, static_types, ...).
pub use leptos_native::renderer::view;

/// Reactive-graph bind helpers re-exported from the renderer.
pub mod reactive_graph {
    pub use leptos_native::renderer::reactive_graph::*;
}

/// GTK-specialized [`IntoView`](leptos_native::IntoView). Pinning R to
/// [`GtkDom`] lets user code write `impl IntoView` (the type parameter
/// is resolved at the trait boundary).
pub trait IntoView: leptos_native::IntoView<GtkDom> {}
impl<T: leptos_native::IntoView<GtkDom>> IntoView for T {}

/// User prelude — items end-user examples bring into scope.
pub mod prelude {
    // Re-export the leptos core prelude FIRST so our gtk-specialized
    // overrides below shadow it (specifically `IntoView`).
    // (`IntoAttributeValue` lives in `common/leptos` and comes in via
    // `core::prelude::*`.)
    pub use crate::core::prelude::*;

    pub use crate::IntoView;

    // Type-erased view container — see crate-level `AnyView` alias.
    pub use crate::AnyView;
    pub use crate::ChildrenFn;
    pub use leptos_native::renderer::view::IntoAny;

    // Mounting
    pub use crate::mount::{mount, mount_to_window, run};

    // GTK-flavoured element builders, exposed as bare functions so
    // user code that writes them directly (instead of via the
    // `view!{}` macro) just works.
    pub use crate::gtk::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        decoration::WithDecoration,
        element::{
            button, checkbox, grid, hstack, label, pop_up_button,
            secure_text_field, slider, stack, stack_view, text_field, toggle,
            vstack,
        },
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
    pub use leptos_native::renderer::menu::Modifiers;

    // Explicit window builder — needed when `run()` (not
    // `mount_to_window`) is used to compose a `<menu_bar>` + window
    // tuple at the top level.
    pub use crate::gtk::window::{window, Window};

    pub use leptos_native::renderer::{
        auto, fit_content, fr, length, max_content, min_content, minmax,
        percent, repeat,
    };
    pub use leptos_native::renderer::attrs::{auto_line, span, GridLine, Overflow};

    pub use crate::GtkDom;
}
