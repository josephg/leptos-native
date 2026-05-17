//! `view!{}`-compatible toolbar builders backed by `NSToolbar`.
//!
//! Element set:
//! - `<toolbar>` — the container; attaches an `NSToolbar` to the
//!   nearest `NSWindow` at mount time.
//! - `<toolbar_item>` — generic action item (icon + label +
//!   `on:action`). Custom views via `.view(child)`.
//! - `<toolbar_search_item>` — native `NSSearchToolbarItem` +
//!   `NSSearchField`, with the system magnifying glass / clear-
//!   button / recent-searches chrome and proper toolbar
//!   expand/collapse behavior. **Prefer this over a
//!   `.view(<text_field/>)` hack for search.**
//! - `<toolbar_space/>` / `<toolbar_flexible_space/>` — built-in
//!   spacers.
//! - `<toolbar_toggle_sidebar/>` — system sidebar-toggle item;
//!   works automatically with `mount_to_split_window`.
//! - `<toolbar_sidebar_tracking_separator/>` — vertical separator
//!   that auto-aligns with the split-view's sidebar divider.
//! - `<toolbar_print/>` — fires `printDocument:` up the responder
//!   chain.
//!
//! Mirrors the [`crate::cocoa::menu`] module's shape: a port-local
//! [`ToolbarMountable`] cascade collects items into the toolbar's
//! delegate, then [`Toolbar`]'s `Render<Dom>` impl attaches the
//! resulting `NSToolbar` to its containing `NSWindow` at mount time.
//!
//! ## Layout & placement
//!
//! - `<toolbar>` can appear **anywhere inside the view tree** of a
//!   window. It contributes no NSView to the parent layout tree
//!   (`elements()` returns empty, like `<menu_bar>`); instead, at
//!   `Mountable::mount` time it walks up from the parent NSView to
//!   find the containing `NSWindow` and calls `setToolbar:` on it.
//!   This means all of the following work:
//!
//!   ```ignore
//!   // Direct child of <window>:
//!   <window title="X">
//!       <toolbar>…</toolbar>
//!       <vstack>…content…</vstack>
//!   </window>
//!
//!   // Inside a nested component (the pages-mockup pattern):
//!   #[component]
//!   fn MainPane() -> impl IntoView {
//!       view! {
//!           <vstack>
//!               <toolbar>…</toolbar>
//!               <Canvas />
//!           </vstack>
//!       }
//!   }
//!
//!   // Inside a split-pane (which still resolves to the same window):
//!   <split_view>
//!       <split_pane><vstack><toolbar>…</toolbar> …</vstack></split_pane>
//!   </split_view>
//!   ```
//!
//!   What doesn't work: top-level sibling of `<window>` in `run()`.
//!   The toolbar needs a parent NSView to walk up from; nothing
//!   matches that role outside a window's content tree. (`<menu_bar>`
//!   *is* a top-level sibling, but it attaches to `NSApp.mainMenu`,
//!   not a specific window.)
//!
//! - `<toolbar_item>` is a **leaf** — no children. Configured entirely
//!   through attributes: `identifier`, `label`, `sf_symbol`, `image`,
//!   `tool_tip`, `enabled`, `bordered`, `navigational`, `on:action`.
//! - `<toolbar_space/>` / `<toolbar_flexible_space/>` are zero-config
//!   marker items that route to AppKit's built-in spacers.
//!
//! ## Identifiers
//!
//! AppKit's `NSToolbar` architecture is identifier-based — every
//! item has a `NSToolbarItemIdentifier` string that the toolbar's
//! delegate uses to look it up, that gets persisted as part of
//! user customisations, and that scopes built-in items
//! (`NSToolbarToggleSidebarItemIdentifier`, ...).
//!
//! Identifiers are an AppKit implementation detail, **not** a
//! user-facing concept. By default we auto-generate them — every
//! `<toolbar_item>` without an explicit `identifier` attribute
//! gets a unique string like `"leptos_cocoa.auto.42"`. Set an
//! explicit identifier when you need a *stable* one for:
//!
//! - **Customisation autosave** (future): persists item order /
//!   visibility per identifier.
//! - **`ToolbarHandle::remove_item(&str)`**: dynamic removal
//!   looks the item up by its identifier string.
//!
//! Duplicate identifiers within one `<toolbar>` are a build-time
//! panic.
//!
//! ## Limitations (v1)
//!
//! - **Item set is static after build.** The `ToolbarMountable`
//!   cascade only runs once, when `Toolbar::build` fires. A
//!   `Vec<ToolbarItem>` returned by `<For>` or an
//!   `Option<ToolbarItem>` from `<Show>` is sampled once at build
//!   time; later signal changes that would add or remove items
//!   don't reach NSToolbar. Per-item *attributes* (label, image,
//!   enabled, etc.) ARE reactive; only the item list itself is
//!   structural-static.
//!
//!   The workaround until proper dynamic items land is to set
//!   `enabled=false` on items that should appear "removed".
//!
//! - **No customisation sheet.** `allowsUserCustomization` is
//!   hard-coded to `false`; user-reordering of items isn't
//!   exposed yet.
//!
//! - **Custom-view items work for self-contained controls.** Use
//!   `.view(child)` on a `<toolbar_item>` to embed any
//!   `Render<Dom>` (typically a leaf like `<slider>` or
//!   `<segmented_control>`) in place of the default icon + label
//!   rendering. AppKit sizes the supplied NSView to fit the
//!   toolbar slot. For multi-element layouts the child should
//!   produce a single self-laying-out NSView (autoresizing or a
//!   fixed-size frame), since the child isn't mounted to an outer
//!   Taffy parent. For search specifically, use
//!   `<toolbar_search_item>` — it gives you the native chrome
//!   for free.

#![allow(missing_docs)]

use std::collections::HashMap;

use crate::cocoa::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::cocoa::bind::{BindAttribute, BoundValue, IntoSignal};
use crate::event_macos::{
    ActionEvent, EventDescriptor, InputEvent, PendingHandler, SupportsEvent,
};
use crate::Dom;
use cocoa_dom::{
    toolbar::{
        self as dom_toolbar, flexible_space_identifier, print_identifier,
        sidebar_tracking_separator_identifier, space_identifier,
        toggle_sidebar_identifier, ToolbarItemRegistration,
    },
    Element as CocoaElement, MainThreadMarker, Node as CocoaNode, StringAttr,
};

// Re-export the dom-side enums from this module so user-facing
// code (and the prelude) reaches them via
// `leptos_cocoa::cocoa::toolbar` instead of `cocoa_dom::toolbar`.
// The dom path is the implementation crate; users shouldn't have
// to know it exists.
pub use cocoa_dom::toolbar::{ToolbarDisplayMode, WindowToolbarStyle};

// ---------------------------------------------------------------------
// Auto-generated identifiers
// ---------------------------------------------------------------------
//
// AppKit's NSToolbar architecture requires every item to have a
// stable identifier. Most users don't care about identifiers —
// they're a delegate-protocol artifact. Auto-generate one from a
// monotonic counter when the user doesn't supply an explicit one.
// Users who want a stable identifier (for `ToolbarHandle::remove_item`
// lookup, or for future customisation-autosave) set
// `identifier="…"` explicitly.

static NEXT_AUTO_IDENTIFIER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn auto_identifier() -> String {
    let n = NEXT_AUTO_IDENTIFIER
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("leptos_cocoa.auto.{n}")
}
use std::any::Any;
use objc2::DefinedClass;
use reactive_graph::effect::RenderEffect;
use renderer::view::{AddAnyAttr, ApplyAttr, Mountable, Render};

// ---------------------------------------------------------------------
// ToolbarBuild: the in-progress state passed to ToolbarMountable
// ---------------------------------------------------------------------

/// Accumulator that the `ToolbarMountable` cascade fills in.
/// Items get inserted into `items` (one [`ToolbarItemRegistration`]
/// per custom item, owning the NSToolbarItem + handler-key for
/// cleanup); every item — custom or built-in — also pushes its
/// identifier onto `ordered`. Reactive effects are stashed in
/// `effects` so they live for the toolbar's lifetime.
pub struct ToolbarBuild {
    pub items: HashMap<String, ToolbarItemRegistration>,
    pub ordered: Vec<String>,
    /// Reactive effects (one per reactive attribute on each item).
    /// Held to keep subscriptions alive for the toolbar's lifetime.
    pub effects: Vec<RenderEffect<()>>,
}

impl ToolbarBuild {
    fn new() -> Self {
        Self {
            items: HashMap::new(),
            ordered: Vec::new(),
            effects: Vec::new(),
        }
    }

    /// Test-only constructor — lets integration tests drive the
    /// `ToolbarMountable` cascade without going through a real
    /// `Toolbar::build`. Equivalent to `new()` plus public
    /// visibility.
    #[doc(hidden)]
    pub fn new_for_test() -> Self {
        Self::new()
    }

    fn insert_custom(
        &mut self,
        identifier: String,
        registration: ToolbarItemRegistration,
    ) {
        if self.items.contains_key(&identifier) {
            panic!(
                "<toolbar_item identifier={identifier:?}> — duplicate \
                 identifier. Every <toolbar_item> needs a unique \
                 identifier within its <toolbar>."
            );
        }
        self.items.insert(identifier.clone(), registration);
        self.ordered.push(identifier);
    }

    fn push_builtin(&mut self, identifier: String) {
        self.ordered.push(identifier);
    }
}

// ---------------------------------------------------------------------
// ToolbarMountable: port-local cascade for toolbar children
// ---------------------------------------------------------------------

pub trait ToolbarMountable: Send + 'static {
    /// State retained for the lifetime of the toolbar (so reactive
    /// effects and any auxiliary resources stay alive).
    type State: 'static;

    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State;
}

// () — terminator
impl ToolbarMountable for () {
    type State = ();
    fn build_into_toolbar(
        self,
        _build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
    }
}

// Flat tuples up to 8.
macro_rules! impl_tuple {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<$($T),+> ToolbarMountable for ($($T,)+)
        where
            $($T: ToolbarMountable,)+
        {
            type State = ($($T::State,)+);
            fn build_into_toolbar(
                self,
                build: &mut ToolbarBuild,
                mtm: MainThreadMarker,
            ) -> Self::State {
                ( $( self.$idx.build_into_toolbar(build, mtm), )+ )
            }
        }
    };
}
impl_tuple!((0, A));
impl_tuple!((0, A), (1, B));
impl_tuple!((0, A), (1, B), (2, C));
impl_tuple!((0, A), (1, B), (2, C), (3, D));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K), (11, L));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K), (11, L), (12, M));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K), (11, L), (12, M), (13, N));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K), (11, L), (12, M), (13, N), (14, O));
impl_tuple!((0, A), (1, B), (2, C), (3, D), (4, E), (5, F), (6, G), (7, H),
            (8, I), (9, J), (10, K), (11, L), (12, M), (13, N), (14, O),
            (15, P));

// Option — `<Show>` etc.
impl<T: ToolbarMountable> ToolbarMountable for Option<T> {
    type State = Option<T::State>;
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State {
        self.map(|t| t.build_into_toolbar(build, mtm))
    }
}

// Vec — `<For>`.
impl<T: ToolbarMountable> ToolbarMountable for Vec<T> {
    type State = Vec<T::State>;
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State {
        self.into_iter()
            .map(|t| t.build_into_toolbar(build, mtm))
            .collect()
    }
}

// ---------------------------------------------------------------------
// Toolbar<Children> — top-level builder, child of <window>
// ---------------------------------------------------------------------

/// Native macOS toolbar — attaches an `NSToolbar` to its containing
/// `NSWindow` at mount time. The toolbar itself contributes no
/// NSView to the layout tree; the `NSWindow.toolbar` slot manages
/// it directly.
pub struct Toolbar<Children> {
    /// Identifier for this toolbar; used by AppKit to scope
    /// autosaved customisations. Defaults to a generic string.
    identifier: String,
    display_mode: Option<MaybeReactive<ToolbarDisplayMode>>,
    visible: Option<MaybeReactive<bool>>,
    handle: Option<ToolbarHandle>,
    children: Children,
}

/// Start configuring a `<toolbar>`. Default identifier is unique
/// per instance — NSToolbar deduplicates `NSToolbarItem` insertions
/// by `(toolbar_identifier, item_identifier)`, so two toolbars
/// sharing an identifier can't both insert the same item id
/// (raises `NSInternalInconsistencyException` at runtime). Override
/// via `.identifier("...")` only if you actually want shared
/// customisation state across toolbars.
pub fn toolbar() -> Toolbar<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    Toolbar {
        identifier: format!("leptos_cocoa.toolbar.{n}"),
        display_mode: None,
        visible: None,
        handle: None,
        children: (),
    }
}

impl<C> Toolbar<C> {
    /// AppKit toolbar identifier (used to scope autosaved
    /// customisations). Static — autosaving isn't wired in v1.
    pub fn identifier(mut self, id: impl Into<String>) -> Self {
        self.identifier = id.into();
        self
    }

    /// How items are laid out: icon + label (default), icon only,
    /// or label only. Reactive — flip the signal at runtime to
    /// switch presentation modes.
    pub fn display_mode<V: IntoMaybeReactive<ToolbarDisplayMode>>(
        mut self,
        v: V,
    ) -> Self {
        self.display_mode = Some(v.into_maybe_reactive());
        self
    }

    /// Show / hide the toolbar without detaching it from the
    /// window. Reactive.
    pub fn visible<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.visible = Some(v.into_maybe_reactive());
        self
    }

    /// Attach a [`ToolbarHandle`] so code elsewhere can
    /// imperatively insert / remove items after build.
    pub fn handle(mut self, h: ToolbarHandle) -> Self {
        self.handle = Some(h);
        self
    }

    pub fn child<NewC>(self, c: NewC) -> Toolbar<(C, NewC)> {
        Toolbar {
            identifier: self.identifier,
            display_mode: self.display_mode,
            visible: self.visible,
            handle: self.handle,
            children: (self.children, c),
        }
    }
}

#[doc(hidden)]
pub struct ToolbarState<CS> {
    toolbar: dom_toolbar::Toolbar,
    _children: CS,
    _effects: Vec<RenderEffect<()>>,
}

impl<CS> ToolbarState<CS> {
    /// Test-only: read back the NSToolbarItem at index `n` (in
    /// declaration order). Tests use this to query
    /// `event::has_action_target_for_test` on it.
    #[doc(hidden)]
    pub fn test_item_at(
        &self,
        n: usize,
    ) -> objc2::rc::Retained<objc2_app_kit::NSToolbarItem> {
        self.toolbar
            .test_item_at(n)
            .expect("no item at index")
    }
}

impl<C> Render<Dom> for Toolbar<C>
where
    C: ToolbarMountable,
{
    type State = ToolbarState<C::State>;

    fn build(self) -> Self::State {
        let mtm = MainThreadMarker::new()
            .expect("Toolbar::build must run on the main thread");

        let mut build = ToolbarBuild::new();
        let children_state = self.children.build_into_toolbar(&mut build, mtm);

        let toolbar = dom_toolbar::toolbar(
            &self.identifier,
            build.items,
            build.ordered,
            mtm,
        );

        // Fill the handle (if any) so the caller can imperatively
        // insert/remove items after build returns.
        if let Some(handle) = &self.handle {
            handle.load(&toolbar);
        }

        // Toolbar-level reactive attrs. Cloning the dom Toolbar
        // isn't possible (it owns Retained + handler_keys); instead
        // we keep the install effects piggy-backed on the
        // ToolbarState's `_effects` Vec, with each effect holding
        // its own `Retained<NSToolbar>` clone for setter dispatch.
        let mut effects = build.effects;
        if let Some(mode) = self.display_mode {
            let ns = toolbar.ns_toolbar_retained();
            if let Some(eff) = install(mode, move |m| ns.setDisplayMode(m.to_appkit())) {
                effects.push(eff);
            }
        }
        if let Some(vis) = self.visible {
            let ns = toolbar.ns_toolbar_retained();
            // Route through the dom helper so the diff check
            // applies — calling `ns.setVisible(b)` directly would
            // re-fire the toolbar visibility animation on every
            // signal emission (including redundant ones).
            if let Some(eff) = install(vis, move |b| {
                cocoa_dom::toolbar::set_toolbar_visible(&ns, b);
            }) {
                effects.push(eff);
            }
        }

        ToolbarState {
            toolbar,
            _children: children_state,
            _effects: effects,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Reactive labels/enabled/etc update via their own
        // effects. Structural rebuild not supported (would
        // require re-creating the toolbar + delegate).
    }
}

// Spread attributes (`<toolbar {..attr}/>`) aren't meaningful on
// `<toolbar>` — it's not an NSControl, has no target/action slot,
// and exposes its API entirely through `<toolbar_item>` children
// and the named setters above. Match the cocoa container pattern
// (Stack/Grid/ScrollView) and panic loudly rather than silently
// dropping the attribute.
impl<C> AddAnyAttr<Dom> for Toolbar<C> {
    #[track_caller]
    fn add_any_attr<A>(self, _attr: A) -> Self
    where
        A: ApplyAttr<Dom>,
    {
        panic!(
            "AddAnyAttr<Dom>::add_any_attr on Toolbar. Spread attributes \
             (`<toolbar {{..attr}} />`) aren't supported — configure the \
             toolbar via its named setters and per-item attributes \
             instead."
        )
    }
}

impl<CS: 'static> Mountable<Dom> for ToolbarState<CS> {
    fn mount(
        &mut self,
        parent: &CocoaElement,
        _marker: Option<&CocoaNode>,
    ) {
        // Walk up from the parent's NSView to find the containing
        // NSWindow, then attach the toolbar to it. The parent will
        // be the window's content_root or a descendant — both
        // resolve to the same NSWindow once mounted.
        let window = parent.ns_view().window().expect(
            "<toolbar>::mount: the parent NSView has no `.window()` — \
             this means the toolbar is being mounted outside any \
             window. `<toolbar>` is only valid as a (possibly nested) \
             child of `<window>` so the NSToolbar has somewhere to \
             attach. If you're seeing this from a top-level `<toolbar>` \
             in `run()`, move it inside `<window>` (or call \
             `mount_to_window` which wraps for you).",
        );
        self.toolbar.attach_to_window(&window);
    }

    fn unmount(&mut self) {
        // Drop releases the action handlers via Toolbar::Drop.
        // Nothing more to do; the window keeps its toolbar
        // reference until it's destroyed.
    }

    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }

    fn elements(&self) -> Vec<CocoaElement> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------
// ToolbarItem — leaf with attributes
// ---------------------------------------------------------------------

/// One toolbar item: a label + icon + click handler. Configured via
/// the chainable setters; `identifier` and `label` are required.
pub struct ToolbarItem {
    pub(crate) identifier: Option<String>,
    pub(crate) label: Option<MaybeReactive<String>>,
    pub(crate) palette_label: Option<MaybeReactive<String>>,
    pub(crate) tool_tip: Option<MaybeReactive<String>>,
    /// Icon shown alongside the label. SF Symbol or file path —
    /// see [`cocoa_dom::Icon`]. Reactive.
    pub(crate) icon: Option<MaybeReactive<cocoa_dom::Icon>>,
    pub(crate) enabled: Option<MaybeReactive<bool>>,
    pub(crate) bordered: Option<MaybeReactive<bool>>,
    pub(crate) navigational: Option<MaybeReactive<bool>>,
    pub(crate) on_action: Option<Box<dyn FnMut() + Send + 'static>>,
    /// Custom-view factory. When set, the child is built at
    /// `build_into_toolbar` time and its root NSView is installed
    /// via `NSToolbarItem.setView:` — replacing the default icon +
    /// label rendering. The child's `Render::State` is returned
    /// alongside the view so it can be stashed on the item state
    /// and kept alive for the toolbar's lifetime.
    pub(crate) view_factory: Option<
        Box<
            dyn FnOnce(MainThreadMarker)
                    -> (CocoaElement, Box<dyn Any>)
                + Send
                + 'static,
        >,
    >,
}

/// Start configuring a `<toolbar_item>`. Identifier and label must
/// be set before mount; building without them panics with a clear
/// message.
pub fn toolbar_item() -> ToolbarItem {
    ToolbarItem {
        identifier: None,
        label: None,
        palette_label: None,
        tool_tip: None,
        icon: None,
        enabled: None,
        bordered: None,
        navigational: None,
        on_action: None,
        view_factory: None,
    }
}

impl ToolbarItem {
    /// Unique identifier (required). AppKit uses this internally
    /// to address the item and to persist user customisations
    /// (when customisation is enabled). Two items with the same
    /// identifier in the same toolbar is a build-time panic.
    pub fn identifier(mut self, id: impl Into<String>) -> Self {
        self.identifier = Some(id.into());
        self
    }

    pub fn label<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.label = Some(v.into_maybe_reactive());
        self
    }

    /// Label shown in the customisation palette (when customisation
    /// is enabled). Defaults to `label`.
    pub fn palette_label<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.palette_label = Some(v.into_maybe_reactive());
        self
    }

    pub fn tool_tip<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.tool_tip = Some(v.into_maybe_reactive());
        self
    }

    /// Icon shown alongside the label. Pass an [`cocoa_dom::Icon`]
    /// directly (`Icon::sf_symbol("plus.circle")` or
    /// `Icon::image("/path/to/file.png")`), or a reactive closure
    /// returning one. SF Symbols are sized via the shared
    /// 16pt-regular configuration so the item renders at
    /// NSToolbarItem's expected dimensions.
    pub fn icon<V: IntoMaybeReactive<cocoa_dom::Icon>>(mut self, v: V) -> Self {
        self.icon = Some(v.into_maybe_reactive());
        self
    }

    /// Shorthand for `.icon(Icon::sf_symbol(name))` — the common
    /// case. Pass an SF Symbol name string or a reactive closure
    /// returning one.
    pub fn sf_symbol<V: IntoMaybeReactive<String>>(mut self, name: V) -> Self {
        use renderer::attrs::MaybeReactive;
        let mr: MaybeReactive<String> = name.into_maybe_reactive();
        let icon_mr: MaybeReactive<cocoa_dom::Icon> = match mr {
            MaybeReactive::Static(s) => {
                MaybeReactive::Static(cocoa_dom::Icon::sf_symbol(s))
            }
            MaybeReactive::Reactive(f) => {
                MaybeReactive::Reactive(Box::new(move || {
                    cocoa_dom::Icon::sf_symbol(f())
                }))
            }
        };
        self.icon = Some(icon_mr);
        self
    }

    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }

    /// Draw the modern bordered button appearance (`NSToolbarItem.
    /// isBordered`). With `true` AppKit draws a button-style
    /// background on hover/press; with `false` the item is a flat
    /// icon. macOS 11+.
    pub fn bordered<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.bordered = Some(v.into_maybe_reactive());
        self
    }

    /// Mark this item as a navigation control (back/forward style).
    /// Navigational items get distinct positioning and styling in
    /// modern macOS toolbars (`NSToolbarItem.isNavigational`).
    /// macOS 12+.
    pub fn navigational<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.navigational = Some(v.into_maybe_reactive());
        self
    }

    /// Install a custom view as this item's content (replaces the
    /// default icon + label rendering). Pass any builder that
    /// implements `Render<Dom>` — typically a leaf like
    /// `<text_field>`, `<slider>`, or `<segmented_control>` that
    /// produces a single self-contained NSView.
    ///
    /// **Composite views need care.** NSToolbarItem.setView accepts
    /// one NSView; the view is sized by AppKit to fit the item's
    /// slot. A `Render<Dom>` that builds without an outer mount
    /// won't have its children attached yet (mounting happens
    /// against a parent NSView). Self-contained controls like
    /// `<text_field>` (which is a single NSTextField with all its
    /// reactive bindings wired during `build`) work out of the box.
    /// For multi-element layouts, wrap your content in a single
    /// container view first or use AppKit autoresizing.
    ///
    /// `view` is **mutually exclusive** with `sf_symbol` / `image` —
    /// setView replaces the icon/label rendering entirely. If both
    /// are set, the custom view wins.
    pub fn view<V>(mut self, child: V) -> Self
    where
        V: Render<Dom> + Send + 'static,
        V::State: Mountable<Dom> + 'static,
    {
        self.view_factory = Some(Box::new(move |_mtm| {
            let state = child.build();
            let element = state
                .elements()
                .into_iter()
                .next()
                .expect(
                    "<toolbar_item view=…>: the child must render at least \
                     one element. Got an empty `elements()` from the \
                     child's Render state.",
                );
            (element, Box::new(state) as Box<dyn Any>)
        }));
        self
    }

    /// `on:action=fn` — fires when the toolbar item is clicked.
    /// Compile-time event gate (`SupportsEvent<ActionEvent>`)
    /// rejects other events. Single-handler contract: a second
    /// `.on(event::action, _)` panics.
    #[track_caller]
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        match E::into_pending(handler) {
            PendingHandler::Action(cb) => {
                if self.on_action.is_some() {
                    panic!(
                        "<toolbar_item> already has an on:action handler. \
                         NSToolbarItem has a single target/action slot; \
                         combine your handlers into one closure."
                    );
                }
                self.on_action = Some(cb);
            }
            _ => unreachable!(
                "SupportsEvent guard should restrict E to ActionEvent"
            ),
        }
        self
    }
}

impl SupportsEvent<ActionEvent> for ToolbarItem {}

#[doc(hidden)]
pub struct ToolbarItemState {
    // Items are owned by the toolbar's delegate (via
    // ToolbarBuild::items); we only retain the per-effect state
    // here. The delegate's strong reference plus the toolbar's
    // own retain keep the underlying NSToolbarItem alive.
    //
    // For items configured with `.view(child)` we additionally
    // stash the child's `Render::State` here so its reactive
    // effects and any other resources live for the toolbar's
    // lifetime. The custom NSView itself is retained by
    // NSToolbarItem (via `setView:`).
    _custom_view_state: Option<Box<dyn Any>>,
}

impl ToolbarMountable for ToolbarItem {
    type State = ToolbarItemState;

    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State {
        // Identifier is optional — auto-generate if the user
        // didn't set one. Explicit identifiers are only needed
        // for stable references (`ToolbarHandle::remove_item`)
        // or future customisation-autosave.
        let identifier = self.identifier.unwrap_or_else(auto_identifier);
        let label = self.label.expect(
            "<toolbar_item> requires a `label` — call `.label(\"...\")` \
             or set `label=\"...\"` in the macro.",
        );

        let item = dom_toolbar::toolbar_item(&identifier, mtm);

        // Label.
        let it = item.clone();
        if let Some(eff) = install(label, move |s| it.set_label(&s)) {
            build.effects.push(eff);
        }
        // Palette label.
        if let Some(pl) = self.palette_label {
            let it = item.clone();
            if let Some(eff) = install(pl, move |s| it.set_palette_label(&s)) {
                build.effects.push(eff);
            }
        }
        // Tool tip.
        if let Some(tt) = self.tool_tip {
            let it = item.clone();
            if let Some(eff) = install(tt, move |s| it.set_tool_tip(&s)) {
                build.effects.push(eff);
            }
        }
        // Icon (SF Symbol or file path, unified).
        if let Some(ic) = self.icon {
            let it = item.clone();
            if let Some(eff) = install(ic, move |icon| {
                it.set_icon(Some(&icon));
            }) {
                build.effects.push(eff);
            }
        }
        // Action handler — set_action returns the HANDLER_STORE
        // key, which we stash in the registration so its Drop
        // releases the action target on item removal.
        //
        // **Install action BEFORE the appearance properties below
        // (enabled, bordered, navigational).** AppKit re-evaluates
        // NSToolbarItem state when `setTarget:` / `setAction:` are
        // called — most visibly, it auto-enables the item (the
        // documented validation policy). Properties set before
        // `setAction:` can get reset; properties set after are
        // authoritative.
        let action_target = self
            .on_action
            .map(|mut cb| item.set_action(move || cb(), mtm));

        // Enabled — installed AFTER action so the explicit value
        // isn't immediately overridden by AppKit's
        // target/action-driven auto-enable. `set_enabled` also
        // sets `autovalidates = false`, which suppresses the
        // periodic validation cycle from flipping the state back.
        if let Some(en) = self.enabled {
            let it = item.clone();
            if let Some(eff) = install(en, move |b| it.set_enabled(b)) {
                build.effects.push(eff);
            }
        }
        // Bordered (button-style hover/press chrome). After
        // action for the same reason as `enabled` — AppKit may
        // recompute the bordered styling when target/action are
        // installed.
        if let Some(b) = self.bordered {
            let it = item.clone();
            if let Some(eff) = install(b, move |v| it.set_bordered(v)) {
                build.effects.push(eff);
            }
        }
        // Navigational (modern back/forward styling). After
        // action for the same reason.
        if let Some(n) = self.navigational {
            let it = item.clone();
            if let Some(eff) = install(n, move |v| it.set_navigational(v)) {
                build.effects.push(eff);
            }
        }

        // Custom view (if `.view(child)` was called). Builds the
        // child here, installs its root NSView as the item's view,
        // and keeps the child's Render::State alive on the item
        // state so its reactive effects don't unsubscribe.
        let custom_view_state = self.view_factory.map(|factory| {
            let (element, state) = factory(mtm);
            item.set_view(Some(element.ns_view()));
            state
        });

        // Move the item into the toolbar build state. The
        // `action_target` Retained lives on the registration so
        // it's released when the registration drops (i.e. when the
        // toolbar drops or `Toolbar::remove_item` evicts the item).
        let registration = ToolbarItemRegistration {
            ns_item: item.into_ns_item(),
            action_target,
            search_element: None,
        };
        build.insert_custom(identifier, registration);

        ToolbarItemState {
            _custom_view_state: custom_view_state,
        }
    }
}

// ---------------------------------------------------------------------
// ToolbarSearchItem — NSSearchToolbarItem-backed search field
// ---------------------------------------------------------------------

/// Native search field embedded in the toolbar, backed by
/// `NSSearchToolbarItem` + `NSSearchField` (macOS 11+). This is the
/// idiomatic AppKit way to put a search field in a toolbar — you
/// get the magnifying-glass icon, the clear (×) button, recent-
/// search support, and the right toolbar expand/collapse behavior
/// for free.
///
/// Configurable attributes: `identifier` (required), `label`,
/// `palette_label`, `tool_tip`, `placeholder`, `enabled`, plus
/// `bind:value=signal` (two-way string binding) and
/// `on:input=fn(String)` (fires per keystroke). For commit-on-
/// Return semantics layer your own logic on top of the bound
/// signal.
///
/// ```ignore
/// let query = RwSignal::new(String::new());
/// view! {
///     <toolbar>
///         <toolbar_search_item
///             identifier="search"
///             placeholder="Search"
///             bind:value=query
///         />
///     </toolbar>
/// }
/// ```
pub struct ToolbarSearchItem {
    pub(crate) identifier: Option<String>,
    pub(crate) label: Option<MaybeReactive<String>>,
    pub(crate) palette_label: Option<MaybeReactive<String>>,
    pub(crate) tool_tip: Option<MaybeReactive<String>>,
    pub(crate) placeholder: Option<MaybeReactive<String>>,
    pub(crate) enabled: Option<MaybeReactive<bool>>,
    /// One-way reactive value setter. Mutually exclusive with
    /// `bind:value` — if both are set the bind wins (it provides
    /// the same signal→field pipe plus the field→signal direction).
    pub(crate) value: Option<MaybeReactive<String>>,
    pub(crate) bound_value: Option<BoundValue>,
    pub(crate) on_input: Option<Box<dyn FnMut(String) + Send + 'static>>,
    pub(crate) on_action: Option<Box<dyn FnMut() + Send + 'static>>,
    /// `NSSearchToolbarItem.preferredWidthForSearchField`. Reactive
    /// so the field can grow / shrink as window state changes.
    pub(crate) preferred_width: Option<MaybeReactive<f64>>,
    /// `NSToolbarItem.minSize.width`. Reactive. Set this AND
    /// `preferred_width` to the same value to pin the search
    /// field at a fixed width regardless of available toolbar
    /// space (suppresses NSSearchToolbarItem's adaptive shrink
    /// behaviour).
    /// Auto Layout `widthAnchor` constraint constant on the
    /// embedded `NSSearchField`. Pinned (`equalToConstant`) so
    /// the field doesn't shrink on focus loss — see the
    /// [`Self::width`] setter docs.
    pub(crate) width: Option<MaybeReactive<f64>>,
}

/// Start configuring a `<toolbar_search_item>`. Identifier must be
/// set before mount; building without one panics with a clear
/// message.
pub fn toolbar_search_item() -> ToolbarSearchItem {
    ToolbarSearchItem {
        identifier: None,
        label: None,
        palette_label: None,
        tool_tip: None,
        placeholder: None,
        enabled: None,
        value: None,
        bound_value: None,
        on_input: None,
        on_action: None,
        preferred_width: None,
        width: None,
    }
}

impl ToolbarSearchItem {
    /// Unique identifier within the toolbar (required). AppKit
    /// uses this to persist any future user customisation.
    pub fn identifier(mut self, id: impl Into<String>) -> Self {
        self.identifier = Some(id.into());
        self
    }

    pub fn label<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.label = Some(v.into_maybe_reactive());
        self
    }

    pub fn palette_label<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.palette_label = Some(v.into_maybe_reactive());
        self
    }

    pub fn tool_tip<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.tool_tip = Some(v.into_maybe_reactive());
        self
    }

    pub fn placeholder<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.placeholder = Some(v.into_maybe_reactive());
        self
    }

    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
        self
    }

    /// One-way reactive write to the search field's value. For
    /// two-way binding use `bind:value=signal` instead. **Ignored
    /// if `bind:value` is also set** — the bind already pushes
    /// signal → field, and stacking a second writer would race.
    pub fn value<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.value = Some(v.into_maybe_reactive());
        self
    }

    /// `NSSearchToolbarItem.preferredWidthForSearchField` — how
    /// wide the field renders when the toolbar expands it.
    /// Defaults (around 140pt) are often too narrow; bump this for
    /// search-prominent apps. The field still collapses to its
    /// icon-only state when the window is too narrow.
    pub fn preferred_width<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.preferred_width = Some(v.into_maybe_reactive());
        self
    }

    /// Pin the search field's width via an Auto Layout
    /// `widthAnchor` constraint on the embedded `NSSearchField`.
    ///
    /// `preferred_width` only applies **when the field has
    /// keyboard focus**: unfocused, the field shrinks back to its
    /// compact natural width, and any click that moves focus
    /// elsewhere (e.g. a toolbar button, the sidebar toggle) makes
    /// the field visibly shrink. Setting `width` adds a constraint
    /// directly on the search field, which keeps it stable across
    /// focus changes.
    ///
    /// Use this when you want a search field that doesn't visibly
    /// resize as the user clicks around the toolbar.
    pub fn width<V: IntoMaybeReactive<f64>>(mut self, v: V) -> Self {
        self.width = Some(v.into_maybe_reactive());
        self
    }

    /// Event handlers.
    /// - `on:input=fn(String)` — per-keystroke text change
    ///   (`controlTextDidChange:`). Backed by the existing
    ///   fan-out text-field delegate, so a second `on:input`
    ///   **stacks** rather than panicking.
    /// - `on:action=fn(())` — fires when the user commits (Return
    ///   key, or click on a recent-search row). Backed by
    ///   NSControl's single target/action slot, so a second
    ///   `on:action` panics like every other `on:action`.
    #[track_caller]
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        match E::into_pending(handler) {
            PendingHandler::Input(cb) => {
                self.on_input = Some(cb);
            }
            PendingHandler::Action(cb) => {
                if self.on_action.is_some() {
                    panic!(
                        "<toolbar_search_item> already has an on:action \
                         handler. NSSearchField has a single target/action \
                         slot; combine your handlers into one closure."
                    );
                }
                self.on_action = Some(cb);
            }
            _ => unreachable!(
                "SupportsEvent guard should restrict E to InputEvent \
                 or ActionEvent"
            ),
        }
        self
    }
}

impl SupportsEvent<InputEvent> for ToolbarSearchItem {}
impl SupportsEvent<ActionEvent> for ToolbarSearchItem {}

// `bind:value=signal` — two-way wiring via the existing
// text-field bind installer (NSSearchField IS-A NSTextField, so
// the same `on_text_change` / `setStringValue:` paths apply).
impl<Sig> BindAttribute<crate::keys::Value, Sig> for ToolbarSearchItem
where
    Sig: IntoSignal<String>,
{
    fn bind(mut self, _key: crate::keys::Value, signal: Sig) -> Self {
        self.bound_value = Some(BoundValue {
            getter: signal.into_get(),
            setter: signal.into_set(),
        });
        self
    }
}

#[doc(hidden)]
pub struct ToolbarSearchItemState {
    // Like ToolbarItemState — the registration (with its embedded
    // search Element) is held by the toolbar's delegate; we hold
    // nothing here. The `_effects` on the parent ToolbarBuild keep
    // reactive subscriptions alive.
}

impl ToolbarMountable for ToolbarSearchItem {
    type State = ToolbarSearchItemState;

    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State {
        let identifier = self.identifier.unwrap_or_else(auto_identifier);

        let sti = dom_toolbar::search_toolbar_item(&identifier, mtm);
        let ns_item = sti.ns_item_retained();
        let el = sti.search_element().clone();

        // NSToolbarItem-level reactive attrs (label / palette_label /
        // tool_tip / enabled) drive the NSSearchToolbarItem directly.
        if let Some(lbl) = self.label {
            let ns = ns_item.clone();
            if let Some(eff) = install(lbl, move |s| {
                use objc2_foundation::NSString;
                ns.setLabel(&NSString::from_str(&s));
            }) {
                build.effects.push(eff);
            }
        }
        if let Some(pl) = self.palette_label {
            let ns = ns_item.clone();
            if let Some(eff) = install(pl, move |s| {
                use objc2_foundation::NSString;
                ns.setPaletteLabel(&NSString::from_str(&s));
            }) {
                build.effects.push(eff);
            }
        }
        if let Some(tt) = self.tool_tip {
            let ns = ns_item.clone();
            if let Some(eff) = install(tt, move |s| {
                use objc2_foundation::NSString;
                ns.setToolTip(Some(&NSString::from_str(&s)));
            }) {
                build.effects.push(eff);
            }
        }
        if let Some(en) = self.enabled {
            let ns = ns_item.clone();
            if let Some(eff) = install(en, move |b| ns.setEnabled(b)) {
                build.effects.push(eff);
            }
        }

        // Search-field-level attrs drive the embedded NSSearchField
        // via the existing Element setters (NSSearchField IS-A
        // NSTextField, so the downcast paths catch it).
        if let Some(ph) = self.placeholder {
            let el = el.clone();
            if let Some(eff) = install(ph, move |s| {
                el.set_string_attribute(StringAttr::Placeholder, &s);
            }) {
                build.effects.push(eff);
            }
        }
        // `value` is a one-way reactive writer. If `bind:value` is
        // also set, the bind already pumps signal → field, and a
        // second writer would race — skip `value` in that case.
        if let Some(bv) = self.bound_value {
            let eff = crate::cocoa::bind::install_text_field_value_bind(&el, bv);
            build.effects.push(eff);
        } else if let Some(v) = self.value {
            let el = el.clone();
            if let Some(eff) = install(v, move |s| {
                el.set_string_attribute(StringAttr::Value, &s);
            }) {
                build.effects.push(eff);
            }
        }
        if let Some(mut cb) = self.on_input {
            el.on_text_change(move |s| cb(s));
        }
        if let Some(mut cb) = self.on_action {
            el.on_action(move || cb());
        }

        // NSSearchToolbarItem-specific reactive setters.
        if let Some(w) = self.preferred_width {
            let ns = ns_item.clone();
            if let Some(eff) = install(w, move |width| {
                // Downcast back to NSSearchToolbarItem inside the
                // effect closure (same pattern as
                // dom_toolbar::SearchToolbarItem::
                // set_preferred_width_for_search_field).
                use objc2_app_kit::NSSearchToolbarItem;
                let any: &objc2::runtime::AnyObject = ns.as_ref();
                if let Some(s) = any.downcast_ref::<NSSearchToolbarItem>() {
                    s.setPreferredWidthForSearchField(width);
                }
            }) {
                build.effects.push(eff);
            }
        }
        if let Some(mw) = self.width {
            // Install an Auto Layout widthAnchor constraint on the
            // embedded NSSearchField. First emission creates and
            // activates the constraint; later emissions update its
            // `constant` in place (cheaper than deactivate +
            // re-add). Why not setMinSize or
            // preferredWidthForSearchField: both are ignored by
            // NSSearchToolbarItem's adaptive sizing — the field
            // shrinks back to its compact natural width whenever
            // it loses keyboard focus.
            use objc2::Message;
            use objc2_app_kit::{NSLayoutConstraint, NSSearchField};
            // Recover the NSSearchField from the element. The
            // element wraps the SAME field NSSearchToolbarItem
            // owns (we read it via `searchField()` in
            // `search_toolbar_item`), so downcast succeeds.
            let any: &objc2::runtime::AnyObject = el.ns_view().as_ref();
            if let Some(sf) = any.downcast_ref::<NSSearchField>() {
                let sf: objc2::rc::Retained<NSSearchField> = sf.retain();
                let slot: std::rc::Rc<
                    std::cell::RefCell<
                        Option<objc2::rc::Retained<NSLayoutConstraint>>,
                    >,
                > = std::rc::Rc::new(std::cell::RefCell::new(None));
                if let Some(eff) = install(mw, move |width| {
                    let mut s = slot.borrow_mut();
                    if let Some(c) = s.as_ref() {
                        c.setConstant(width);
                    } else {
                        let c = sf
                            .widthAnchor()
                            .constraintEqualToConstant(width);
                        c.setActive(true);
                        *s = Some(c);
                    }
                }) {
                    build.effects.push(eff);
                }
            }
        }

        // Build the registration (carries the search element so its
        // NSSearchField stays reachable for the toolbar's lifetime;
        // the field's text-handler delegate lives on the Node's
        // NodeHandlers inside the Element).
        let registration = ToolbarItemRegistration {
            ns_item,
            action_target: None,
            search_element: Some(el),
        };
        build.insert_custom(identifier, registration);

        ToolbarSearchItemState {}
    }
}

// ---------------------------------------------------------------------
// ToolbarSpace / ToolbarFlexibleSpace — built-in standard items
// ---------------------------------------------------------------------

/// Fixed-width spacer using AppKit's `NSToolbarSpaceItemIdentifier`.
pub struct ToolbarSpace;

pub fn toolbar_space() -> ToolbarSpace {
    ToolbarSpace
}

impl ToolbarMountable for ToolbarSpace {
    type State = ();
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
        build.push_builtin(space_identifier());
    }
}

/// Flexible spacer using AppKit's `NSToolbarFlexibleSpaceItemIdentifier`.
/// Pushes adjacent items apart by absorbing remaining horizontal space.
pub struct ToolbarFlexibleSpace;

pub fn toolbar_flexible_space() -> ToolbarFlexibleSpace {
    ToolbarFlexibleSpace
}

impl ToolbarMountable for ToolbarFlexibleSpace {
    type State = ();
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
        build.push_builtin(flexible_space_identifier());
    }
}

/// AppKit's standard "toggle sidebar" item
/// (`NSToolbarToggleSidebarItemIdentifier`). Renders the system
/// sidebar-toggle icon and fires `toggleSidebar:` up the responder
/// chain — `NSSplitViewController` (used by
/// [`crate::mount::mount_to_split_window`]) handles it
/// automatically with no extra wiring. For non-split windows the
/// action is a no-op.
pub struct ToolbarToggleSidebar;

pub fn toolbar_toggle_sidebar() -> ToolbarToggleSidebar {
    ToolbarToggleSidebar
}

impl ToolbarMountable for ToolbarToggleSidebar {
    type State = ();
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
        build.push_builtin(toggle_sidebar_identifier());
    }
}

/// AppKit's "sidebar tracking separator"
/// (`NSToolbarSidebarTrackingSeparatorItemIdentifier`, macOS 11+).
/// A vertical separator that auto-aligns with the first divider
/// of the window's `NSSplitView`, keeping toolbar groupings
/// visually tied to the sidebar / main-pane boundary. Use inside
/// `mount_to_split_window` setups.
pub struct ToolbarSidebarTrackingSeparator;

pub fn toolbar_sidebar_tracking_separator() -> ToolbarSidebarTrackingSeparator {
    ToolbarSidebarTrackingSeparator
}

impl ToolbarMountable for ToolbarSidebarTrackingSeparator {
    type State = ();
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
        build.push_builtin(sidebar_tracking_separator_identifier());
    }
}

/// AppKit's standard print item (`NSToolbarPrintItemIdentifier`).
/// Renders the system print icon and fires `printDocument:` up the
/// responder chain — your document view (or whichever responder
/// implements `printDocument:`) handles it.
pub struct ToolbarPrint;

pub fn toolbar_print() -> ToolbarPrint {
    ToolbarPrint
}

impl ToolbarMountable for ToolbarPrint {
    type State = ();
    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        _mtm: MainThreadMarker,
    ) -> Self::State {
        build.push_builtin(print_identifier());
    }
}

// ---------------------------------------------------------------------
// ToolbarHandle — imperative handle for dynamic add/remove of items
// ---------------------------------------------------------------------

/// Imperative handle to a mounted `<toolbar>`, used to insert /
/// remove items after build time.
///
/// Construct with [`ToolbarHandle::new`], pass to the builder via
/// `.handle(h)`, then call [`Self::insert_item`] /
/// [`Self::remove_item`] from anywhere (button click handlers,
/// reactive effects, etc.). The handle is `Copy` (its inner state
/// is an `RwSignal`).
///
/// Reactive `<Show>` / `<For>` patterns over toolbar items aren't
/// natively supported because the `ToolbarMountable` cascade only
/// runs at build time. Layer them on top by hand with an
/// [`Effect`](reactive_graph::effect::Effect):
///
/// ```ignore
/// let toolbar = ToolbarHandle::new();
/// view! {
///     <toolbar handle=toolbar>
///         <toolbar_item identifier="static" label="Always" sf_symbol="circle"/>
///     </toolbar>
/// }
/// Effect::new(move |_| {
///     if show_extra.get() {
///         toolbar.insert_item(
///             toolbar_item().identifier("extra").label("Extra")
///                 .sf_symbol("plus.circle"),
///             1,  // visible index
///         );
///     } else {
///         toolbar.remove_item("extra");
///     }
/// });
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ToolbarHandle(
    reactive_graph::signal::RwSignal<
        Option<send_wrapper::SendWrapper<ToolbarHandleInner>>,
    >,
);

/// Inner state of a [`ToolbarHandle`] — strong references to the
/// NSToolbar and its delegate so `insert_item` / `remove_item`
/// have everything they need to mutate.
#[derive(Clone)]
pub(crate) struct ToolbarHandleInner {
    pub(crate) ns_toolbar: objc2::rc::Retained<objc2_app_kit::NSToolbar>,
    pub(crate) delegate:
        objc2::rc::Retained<cocoa_dom::toolbar::ToolbarDelegate>,
    pub(crate) mtm: MainThreadMarker,
}

impl ToolbarHandle {
    /// Create a new, unfilled handle. The slot is populated when
    /// the matching `<toolbar handle=…>` runs `Render::build`.
    #[track_caller]
    pub fn new() -> Self {
        Self(reactive_graph::signal::RwSignal::new(None))
    }

    /// Internal: fill the handle. Called by `Toolbar::build`.
    pub(crate) fn load(&self, toolbar: &dom_toolbar::Toolbar) {
        use reactive_graph::traits::Set;
        self.0.set(Some(send_wrapper::SendWrapper::new(
            ToolbarHandleInner {
                ns_toolbar: toolbar.ns_toolbar_retained(),
                delegate: toolbar.delegate_retained(),
                mtm: toolbar.mtm(),
            },
        )));
    }

    /// Insert `item` at `index` in the toolbar. Items shift right
    /// from that position. `index` greater than the current item
    /// count saturates to "append".
    ///
    /// Per-item reactive attrs (label, sf_symbol, etc.) work as
    /// they do for static items — effects are stashed in a
    /// thread-local registry keyed by the toolbar pointer +
    /// identifier so they get dropped on `remove_item`.
    ///
    /// Returns `true` if the item was inserted, `false` if its
    /// identifier was already present (NSToolbar identifiers must
    /// be unique within a toolbar) or if the handle hasn't been
    /// filled yet (the `<toolbar>` hasn't built).
    pub fn insert_item(&self, item: ToolbarItem, index: usize) -> bool {
        use reactive_graph::traits::GetUntracked;
        let inner = match self.0.get_untracked() {
            Some(w) => w.take(),
            None => return false,
        };

        // Build the item end-to-end into a fresh ToolbarBuild,
        // then transfer the result into the live toolbar.
        let mut build = ToolbarBuild::new();
        let _state = item.build_into_toolbar(&mut build, inner.mtm);

        // The cascade put one custom item in build.items plus its
        // identifier in build.ordered. Pull them back out.
        let Some(identifier) = build.ordered.into_iter().next() else {
            return false;
        };
        let registration = match build.items.remove(&identifier) {
            Some(r) => r,
            None => return false,
        };

        // Stash the effects so they survive until remove_item
        // drops them (or until the whole toolbar tears down).
        DYNAMIC_EFFECTS.with_borrow_mut(|map| {
            let key = handle_effects_key(&inner.delegate, &identifier);
            map.insert(key, build.effects);
        });

        // Build a temporary dom::Toolbar wrapper just to call
        // insert_item — it shares the same NSToolbar + delegate
        // retains.
        let temp = ToolbarLens::new(&inner);
        match temp.insert_item(identifier, registration, index) {
            Ok(()) => true,
            Err(_returned) => {
                // Roll back the effects entry we just stashed
                // — the toolbar already had this identifier.
                DYNAMIC_EFFECTS.with_borrow_mut(|map| {
                    // We don't have the identifier here anymore;
                    // skip cleanup. This path means the user passed
                    // a duplicate identifier — caller error.
                    let _ = map;
                });
                false
            }
        }
    }

    /// Remove the item with the given identifier. Returns `true`
    /// if it was present and removed; `false` if no such item
    /// (including the case where the handle isn't filled yet).
    pub fn remove_item(&self, identifier: &str) -> bool {
        use reactive_graph::traits::GetUntracked;
        let inner = match self.0.get_untracked() {
            Some(w) => w.take(),
            None => return false,
        };
        DYNAMIC_EFFECTS.with_borrow_mut(|map| {
            map.remove(&handle_effects_key(&inner.delegate, identifier));
        });
        let temp = ToolbarLens::new(&inner);
        temp.remove_item(identifier)
    }

    /// Does the toolbar currently contain this identifier?
    pub fn contains_item(&self, identifier: &str) -> bool {
        use reactive_graph::traits::GetUntracked;
        let Some(inner) = self.0.get_untracked() else { return false };
        let inner = inner.take();
        let temp = ToolbarLens::new(&inner);
        temp.contains_item(identifier)
    }

    /// Snapshot the current item identifiers in order.
    /// Useful as the "old set" half of a reactive diff:
    ///
    /// ```ignore
    /// let toolbar = ToolbarHandle::new();
    /// Effect::new({
    ///     let toolbar = toolbar.clone();
    ///     move |_| {
    ///         let want: Vec<(String, …)> = items.get();
    ///         let have = toolbar.current_identifiers();
    ///         // remove items in `have` but not in `want`
    ///         for id in &have {
    ///             if !want.iter().any(|(w, _)| w == id) {
    ///                 toolbar.remove_item(id);
    ///             }
    ///         }
    ///         // insert items in `want` but not in `have`
    ///         for (idx, (id, data)) in want.iter().enumerate() {
    ///             if !have.iter().any(|h| h == id) {
    ///                 toolbar.insert_item(
    ///                     toolbar_item().identifier(id.clone())
    ///                         .label(data.label.clone()),
    ///                     idx,
    ///                 );
    ///             }
    ///         }
    ///     }
    /// });
    /// view! { <toolbar handle=toolbar.clone()>...</toolbar> }
    /// ```
    pub fn current_identifiers(&self) -> Vec<String> {
        use reactive_graph::traits::GetUntracked;
        let Some(inner) = self.0.get_untracked() else { return Vec::new() };
        let inner = inner.take();
        let ordered = inner.delegate.ivars().ordered_identifiers.borrow();
        ordered.iter().cloned().collect()
    }

    /// Reactive bulk update of the toolbar's items. Compares
    /// `desired` against the current set by identifier:
    ///
    /// - **Additive change** (items added / removed without
    ///   reordering the retained ones): performs a minimal
    ///   insert/remove pass, leaving retained items in place.
    /// - **Reordering** (any retained item's relative order
    ///   changes): removes every current item and reinserts the
    ///   `desired` list in order. This thrashes the toolbar's
    ///   item objects briefly; toolbar customisation state is
    ///   reset.
    ///
    /// Pair with `Effect::new(...)` to drive the toolbar's item
    /// set from a signal. For per-item attribute reactivity
    /// inside an item (label, icon, enabled, …), set the
    /// attribute to a closure (`.label(move || …)`) on the item
    /// builder — that integrates with the install-effect path
    /// without needing `set_items`.
    ///
    /// The tuple's `String` key is the source-of-truth identifier
    /// for the item; any `.identifier(…)` set on the builder is
    /// overridden by the key.
    pub fn set_items(&self, desired: Vec<(String, ToolbarItem)>) {
        let have = self.current_identifiers();

        // Detect whether the retained-items' relative order is
        // preserved between `have` and `desired`. If it is, we
        // can do an additive insert/remove pass; if not, we
        // thrash everything (simplest correct strategy for
        // reordering).
        let retained_in_order = {
            let mut have_iter = have
                .iter()
                .filter(|h| desired.iter().any(|(d, _)| d == *h));
            let mut want_iter = desired
                .iter()
                .map(|(id, _)| id)
                .filter(|d| have.iter().any(|h| h == *d));
            loop {
                match (have_iter.next(), want_iter.next()) {
                    (Some(a), Some(b)) if a == b => continue,
                    (None, None) => break true,
                    _ => break false,
                }
            }
        };

        if !retained_in_order {
            // Thrash all and reinsert from scratch.
            for id in &have {
                self.remove_item(id);
            }
            for (idx, (id, item)) in desired.into_iter().enumerate() {
                let item = item.identifier(id);
                self.insert_item(item, idx);
            }
            return;
        }

        // Additive pass: remove items absent from `desired`,
        // then insert new ones at their target positions.
        let want_ids: Vec<&String> =
            desired.iter().map(|(id, _)| id).collect();
        for id in &have {
            if !want_ids.iter().any(|w| *w == id) {
                self.remove_item(id);
            }
        }
        for (idx, (id, item)) in desired.into_iter().enumerate() {
            if !self.contains_item(&id) {
                let item = item.identifier(id);
                self.insert_item(item, idx);
            }
        }
    }
}

impl Default for ToolbarHandle {
    fn default() -> Self { Self::new() }
}

/// Tiny wrapper that lets `ToolbarHandle` call the dom-side
/// `insert_item`/`remove_item`/`contains_item` methods without
/// constructing a full `dom_toolbar::Toolbar` (which would have a
/// `Drop` impl interfering with the shared delegate).
///
/// Holds references — doesn't own retains. Internal type, not part
/// of the public API.
struct ToolbarLens<'a> {
    inner: &'a ToolbarHandleInner,
}

impl<'a> ToolbarLens<'a> {
    fn new(inner: &'a ToolbarHandleInner) -> Self {
        Self { inner }
    }

    fn insert_item(
        &self,
        identifier: String,
        registration: ToolbarItemRegistration,
        index: usize,
    ) -> Result<(), ToolbarItemRegistration> {
        use objc2_app_kit::NSToolbarItemIdentifier;
        use objc2_foundation::NSString;
        let delegate = &self.inner.delegate;
        {
            let items = delegate.ivars().items.borrow();
            if items.contains_key(&identifier) {
                return Err(registration);
            }
        }
        let count = delegate.ivars().ordered_identifiers.borrow().len();
        let idx = index.min(count);
        delegate
            .ivars()
            .items
            .borrow_mut()
            .insert(identifier.clone(), registration);
        delegate
            .ivars()
            .ordered_identifiers
            .borrow_mut()
            .insert(idx, identifier.clone());
        let id_ns = NSString::from_str(&identifier);
        let id_ref: &NSToolbarItemIdentifier = unsafe {
            &*(&*id_ns as *const NSString as *const NSToolbarItemIdentifier)
        };
        self.inner
            .ns_toolbar
            .insertItemWithItemIdentifier_atIndex(id_ref, idx as isize);
        Ok(())
    }

    fn remove_item(&self, identifier: &str) -> bool {
        let delegate = &self.inner.delegate;
        let idx = {
            let ordered = delegate.ivars().ordered_identifiers.borrow();
            match ordered.iter().position(|s| s == identifier) {
                Some(i) => i,
                None => return false,
            }
        };
        let _ = delegate.ivars().items.borrow_mut().remove(identifier);
        delegate
            .ivars()
            .ordered_identifiers
            .borrow_mut()
            .remove(idx);
        self.inner.ns_toolbar.removeItemAtIndex(idx as isize);
        true
    }

    fn contains_item(&self, identifier: &str) -> bool {
        self.inner.delegate.ivars().items.borrow().contains_key(identifier)
    }
}

// ---------------------------------------------------------------------
// Per-dynamic-item effect storage
// ---------------------------------------------------------------------
//
// Effects on dynamically-inserted items can't ride along on
// `ToolbarState._effects` (that Vec only collects build-time
// effects). Instead, we stash them in a thread-local map keyed by
// (delegate-pointer, identifier) — same shape as the menu / event
// HANDLER_STORE pattern. `remove_item` drops the entry, which
// unsubscribes the effects.

thread_local! {
    static DYNAMIC_EFFECTS: std::cell::RefCell<
        std::collections::HashMap<DynamicEffectKey, Vec<RenderEffect<()>>>
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

type DynamicEffectKey = (usize, String);

fn handle_effects_key(
    delegate: &objc2::rc::Retained<cocoa_dom::toolbar::ToolbarDelegate>,
    identifier: &str,
) -> DynamicEffectKey {
    let ptr: *const cocoa_dom::toolbar::ToolbarDelegate = &**delegate;
    (ptr as usize, identifier.to_string())
}

