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
/// shape the `view!{}` macro emits
/// (`::leptos::tachys::html::element::*`, `::leptos::tachys::view::*`).
///
/// The web Leptos macro additionally routed tags whose names are real
/// SVG elements (`<switch>`, ...) through `tachys::svg::*` and emitted
/// `.attr(name, value)` for every attribute. On native there's no SVG
/// renderer and no untyped `.attr()` slot, so this fork's macro routes
/// every tag through `tachys::html::element::*`.
///
/// User code reaching for a builder directly (no `view!{}`) should
/// import it from this path (`leptos::tachys::html::element::button`,
/// ...) or from `leptos::prelude` if the prelude re-exports it.
pub mod tachys {
    pub use ::renderer::view;

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
}

/// Cocoa-specialized [`IntoView`](leptos::IntoView). Pinning R to
/// [`Dom`] lets user code write `impl IntoView` (the type parameter
/// is resolved at the trait boundary) without sprinkling `<Dom>` —
/// or worse, `<R: Renderer>` — through every component signature.
pub trait IntoView: leptos::IntoView<Dom> {}
impl<T: leptos::IntoView<Dom>> IntoView for T {}

/// User prelude — the items end-user examples bring into scope.
///
/// Built around `use leptos::prelude::*;` resolving to this module
/// when the example's `Cargo.toml` aliases `leptos = "leptos_cocoa"`.
/// The contents fall into these groups:
///
/// - **Reactive core** (re-exported from `leptos`): `RwSignal`,
///   `Memo`, `Effect`, `provide_context`/`use_context`, the
///   `#[component]` and `view!{}` macros, `<Show>`, `<Switch>` /
///   `<Match>`, `<For>`, `IntoView`, etc.
/// - **Mounting**: [`mount_to_window`](crate::mount::mount_to_window),
///   [`run`](crate::mount::run).
/// - **Element builders**: `button()`, `vstack()`, `hstack()`,
///   `stack()`, `grid()`, `label()`, `text_field()`.
///   Used directly *or* via the `view!` macro's element syntax.
/// - **Attribute traits**: `WithLayout` / `WithUniversal` / cocoa-
///   port `WithText` for chainable setters
///   (`.padding(8.0).flex_grow(1.0).text_color(c)` etc.).
/// - **Style enums**: `AlignItems`, `JustifyContent`, `FlexDirection`,
///   `FlexWrap`, `Edges`, `Dim`, `AlignSelf`, `GridLine`,
///   `LineBreak`, `TextAlignment`, `Color`.
/// - **Grid helpers**: `fr`, `length`, `auto`, `percent`,
///   `min_content`, `max_content`, `minmax`, `fit_content`,
///   `repeat`, `span`, `auto_line`.
/// - **Native helpers**: `local_storage`, `set_interval`,
///   `KeyEvent`, `Storage`.
///
/// Anything not listed lives at `leptos::core::*` (the inner
/// `common/leptos` crate, re-exported as `core`) or `leptos::reactive::*`
/// (reactive_graph). The prelude only pulls in the day-to-day surface.
pub mod prelude {
    // Re-export the leptos core prelude FIRST so our cocoa-specialized
    // overrides below shadow it (specifically `IntoView`).
    // (`IntoAttributeValue` lives in `common/leptos` and is brought in
    // via `core::prelude::*`.)
    pub use crate::core::prelude::*;

    // Cocoa-specialized IntoView (pinned to Dom; no R param).
    pub use crate::IntoView;

    // Mounting
    pub use crate::mount::{mount_to_split_window, mount_to_window, run};

    // Split-view builders + the pane-behavior enum.
    pub use crate::cocoa::split::{
        split_pane, split_view, CollapseBehavior, PaneBehavior, SplitPane,
        SplitView,
    };

    // Window types (handle, size, position) so user code can
    // construct a programmatic close handle or pass a (Width, Height)
    // tuple via the typed constructors directly.
    pub use crate::cocoa::window::{
        WindowHandle, WindowPosition, WindowSize,
    };

    // Menu builders + portable Modifiers. `<menu_bar>` is a
    // top-level sibling of `<window>` in `run()`; nested `<menu>`s
    // and `<menu_item>`s describe the contents.
    pub use crate::cocoa::menu::{
        menu, menu_bar, menu_item, menu_separator, Menu, MenuBar, MenuItem,
        MenuSeparator,
    };
    pub use renderer::menu::Modifiers;

    // Toolbar builders. `<toolbar>` is a child of `<window>` that
    // attaches an `NSToolbar` to the containing NSWindow at mount
    // time. `<toolbar_item>` is a leaf with attributes (label,
    // sf_symbol, on:action, ...).
    pub use crate::cocoa::toolbar::{
        toolbar, toolbar_flexible_space, toolbar_item, toolbar_print,
        toolbar_search_item, toolbar_sidebar_tracking_separator,
        toolbar_space, toolbar_toggle_sidebar, Toolbar, ToolbarDisplayMode,
        ToolbarFlexibleSpace, ToolbarHandle, ToolbarItem, ToolbarPrint,
        ToolbarSearchItem, ToolbarSidebarTrackingSeparator, ToolbarSpace,
        ToolbarToggleSidebar, WindowToolbarStyle,
    };

    // Cocoa-flavoured element builders, exposed as bare functions
    // (`button()`, `vstack()`, etc.) so user code that writes them
    // directly (instead of via the `view!{}` macro) just works.
    pub use crate::cocoa::{
        attr::{IntoMaybeReactive, MaybeReactive},
        bind::{BindAttribute, IntoSignal},
        element::{
            button, grid, hstack, label, stack, text_field, vstack,
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
            // Same shadow pattern: port-local `WithDecoration` pins
            // `C = Color` for the `IntoMaybeReactive` orphan-rule
            // reasons. Adds `background_color` / `corner_radius` /
            // `border_width` / `border_color` / `clip` to every
            // builder.
            WithDecoration,
        },
        node_ref::NodeRef,
        AlignContent, AlignItems, FlexDirection, FlexWrap, GridAutoFlow,
        GridTemplateComponent, JustifyContent, JustifyItems,
        TrackSizingFunction,
    };

    // Grid track-sizing helpers — re-exported from Taffy via
    // `renderer` so example code can write `[fr(1.0), auto()]`
    // without importing taffy. `span(n)` doubles as a placement
    // helper for child elements (`.grid_column_end(span(2))`).
    pub use renderer::{
        auto, fit_content, fr, length, max_content, min_content, minmax, percent,
        repeat,
    };

    // Grid placement attrs (added to every element via `WithLayout`).
    // Re-exported so user code can pass `GridLine::Auto` explicitly,
    // or use `span(n)` / integer literals via `Into<GridLine>`.
    pub use renderer::attrs::{auto_line, AlignSelf, Dim, Edges, GridLine};

    // Native value types + helpers commonly used by examples (timers,
    // persistent storage, colour, date, key events, text/segment/date
    // styling enums). User code should reach for these from the
    // prelude, never directly from `cocoa_dom` — the implementation
    // crate's path is not stable.
    //
    // `Element` is the imperative handle for directives and NodeRef
    // (`use:directive=fn` calls `fn(el: Element)`; `NodeRef::get()` →
    // `Option<Element>`).
    pub use cocoa_dom::{
        local_storage, set_interval, set_interval_with_handle, Color, Date,
        DatePickerStyle, Element, Icon, IntervalError, IntervalHandle,
        KeyEvent, LineBreak, SegmentStyle, Storage, StorageError, TextAlignment,
    };
    // Programmatic shutdown. Wire to a Quit menu item's on:action,
    // or call from anywhere on the main thread.
    pub use cocoa_dom::app::quit;
    pub use crate::Dom;
}
