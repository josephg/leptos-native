//! View-prelude module — provides `__leptos_view`, the namespace the
//! `view!{}` macro expands to.
//!
//! The macro emits relative paths like `__leptos_view::elements::button()`,
//! `__leptos_view::events::on(__leptos_view::events::click, …)`,
//! `__leptos_view::attrs::id(...)`, `__leptos_view::bind::Selection`. To make
//! these resolve, callers bring `__leptos_view` into scope via
//! `use leptos::view_prelude::*;` (this module re-exports it for the web
//! target). Native users do `use leptos_<backend>::view_prelude::*;` for the
//! same `__leptos_view` name pointing at their renderer's module tree.
//!
//! This module is web-only. On native targets the corresponding view-prelude
//! lives in the per-renderer glue crate (`leptos_cocoa`, `leptos_gtk`,
//! `leptos_ios`).

#![allow(non_camel_case_types)]
#![allow(missing_docs)]

#[doc(hidden)]
#[cfg(feature = "web")]
pub mod __leptos_view {
    /// Element constructors. The `view!` macro emits
    /// `__leptos_view::elements::button()`, `…::doctype(...)`,
    /// `…::InertElement::new(...)`, plus all SVG/MathML/custom tags
    /// — they all flow through this single namespace.
    ///
    /// HTML, SVG, and MathML have several name collisions (e.g.
    /// `script`, `title`, `style`, `view`). We re-export each
    /// namespace, with the HTML version winning by ordering — SVG
    /// glob is included after HTML so its ambient names override,
    /// matching the upstream behavior of the macro's prior dispatch
    /// table (HTML was preferred for ambiguous names). Apps that
    /// need a specific SVG/MathML element shadowed by an HTML name
    /// can import it directly from `tachys::svg` / `tachys::mathml`.
    pub mod elements {
        pub use ::tachys::html::doctype;
        pub use ::tachys::html::element::*;
        pub use ::tachys::html::InertElement;
    }

    /// SVG element constructors. The `view!` macro routes all tags
    /// through `elements::*`; SVG tags that don't collide with HTML
    /// are re-exported from there. Apps that need SVG-specific
    /// resolution (e.g. for `<view>` SVG semantics, `<title>`, etc.)
    /// can import explicitly from this module.
    pub mod svg {
        pub use ::tachys::svg::*;
    }

    /// MathML element constructors. Same pattern as `svg`.
    pub mod mathml {
        pub use ::tachys::mathml::*;
    }

    /// Event types and the `on(...)` / `capture(...)` / `undelegated(...)`
    /// wrappers. The `view!` macro emits `__leptos_view::events::click`
    /// etc., plus `__leptos_view::events::on(ev, handler)`.
    pub mod events {
        pub use ::tachys::html::event::*;
    }

    /// Attribute helpers (id, class, style, property, node_ref,
    /// directive, custom_attribute, …). The `view!` macro routes
    /// every non-`bind:` attribute through this namespace.
    pub mod attrs {
        pub use ::tachys::html::attribute::custom::custom_attribute;
        pub use ::tachys::html::attribute::*;
        pub use ::tachys::html::class::class;
        pub use ::tachys::html::directive::directive;
        pub use ::tachys::html::node_ref::node_ref;
        pub use ::tachys::html::property::prop;
        pub use ::tachys::html::style::style;
    }

    /// `bind:` keys. The `view!` macro emits
    /// `.bind(__leptos_view::bind::Selection, signal)` for
    /// `bind:selection=signal`, and similarly for the other one-way
    /// and two-way bindings. Includes `bind:group` (Group) which
    /// the web target backs via `tachys::reactive_graph::bind::Group`.
    pub mod bind {
        pub use ::tachys::html::attribute::*;
        pub use ::tachys::reactive_graph::bind::Group;
    }
}

// `__leptos_view` is the module name the `view!` macro expects in
// scope. Glob-importing this `view_prelude` (or `leptos::prelude::*`,
// which re-exports it) makes the name visible.

// ----- Native targets (transitional; Phase 5 will move this into per-
// renderer glue crates `leptos_cocoa`/`leptos_ios`/`leptos_gtk`). For
// now the per-OS builders still live in `tachys::cocoa`/`::ios`/`::gtk`,
// so this view_prelude routes there. -----

// macOS native code lives in the `leptos_cocoa` glue crate (Phase 5).
// Users `use leptos_cocoa::view_prelude::*;` for the corresponding
// `__leptos_view` namespace.

// iOS native code lives in the `leptos_ios` glue crate (Phase 5).
// Users `use leptos_ios::view_prelude::*;` for the corresponding
// `__leptos_view` namespace.

// Linux/GTK native code lives in the `leptos_gtk` glue crate
// (Phase 5). Users `use leptos_gtk::view_prelude::*;` for the
// corresponding `__leptos_view` namespace.
