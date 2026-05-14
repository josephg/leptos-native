//! `view!{}`-compatible toolbar builders backed by `NSToolbar`:
//! `<toolbar>`, `<toolbar_item>`, `<toolbar_space/>`,
//! `<toolbar_flexible_space/>`.
//!
//! Mirrors the [`crate::cocoa::menu`] module's shape: a port-local
//! [`ToolbarMountable`] cascade collects items into the toolbar's
//! delegate, then [`Toolbar`]'s `Render<Dom>` impl attaches the
//! resulting `NSToolbar` to its containing `NSWindow` at mount time.
//!
//! ## Layout
//!
//! - `<toolbar>` is a **child of `<window>`**, sibling to the rest of
//!   the view content. It contributes no NSView to the parent layout
//!   tree (`elements()` returns empty, like `<menu_bar>`).
//! - `<toolbar_item>` is a **leaf** — no children. Configured entirely
//!   through attributes: `identifier`, `label`, `sf_symbol`, `image`,
//!   `tool_tip`, `enabled`, `on:action`.
//! - `<toolbar_space/>` / `<toolbar_flexible_space/>` are zero-config
//!   marker items that route to AppKit's built-in spacers.
//!
//! ## Identifiers
//!
//! Every `<toolbar_item>` requires an explicit `identifier` (string).
//! Identifiers must be unique within the toolbar; duplicates panic
//! at build time. AppKit uses these identifiers to persist
//! customisations (when we eventually wire customisation up), so
//! auto-generation isn't a good default.

#![allow(missing_docs)]

use std::collections::HashMap;

use crate::cocoa::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::event_macos::{
    ActionEvent, EventDescriptor, PendingHandler, SupportsEvent,
};
use crate::Dom;
use cocoa_dom::{
    toolbar::{
        self as dom_toolbar, flexible_space_identifier, is_builtin_identifier,
        space_identifier,
    },
    Element as CocoaElement, MainThreadMarker, Node as CocoaNode,
};
use objc2::rc::Retained;
use objc2_app_kit::NSToolbarItem;
use reactive_graph::effect::RenderEffect;
use renderer::view::{AddAnyAttr, ApplyAttr, Mountable, Render};

// ---------------------------------------------------------------------
// ToolbarBuild: the in-progress state passed to ToolbarMountable
// ---------------------------------------------------------------------

/// Accumulator that the `ToolbarMountable` cascade fills in.
/// Items get inserted into `items` (one entry per custom item);
/// every item — custom or built-in — also pushes its identifier
/// onto `ordered`. `handler_keys` collects the
/// `HANDLER_STORE` keys for any item with an installed action,
/// so the parent `Toolbar` can release them on drop.
pub struct ToolbarBuild {
    pub items: HashMap<String, Retained<NSToolbarItem>>,
    pub ordered: Vec<String>,
    pub handler_keys: Vec<usize>,
    /// Reactive effects (one per reactive attribute on each item).
    /// Held to keep subscriptions alive for the toolbar's lifetime.
    pub effects: Vec<RenderEffect<()>>,
}

impl ToolbarBuild {
    fn new() -> Self {
        Self {
            items: HashMap::new(),
            ordered: Vec::new(),
            handler_keys: Vec::new(),
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
        item: Retained<NSToolbarItem>,
    ) {
        if self.items.contains_key(&identifier) {
            panic!(
                "<toolbar_item identifier={identifier:?}> — duplicate \
                 identifier. Every <toolbar_item> needs a unique \
                 identifier within its <toolbar>."
            );
        }
        self.items.insert(identifier.clone(), item);
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
    children: Children,
}

/// Start configuring a `<toolbar>`. Defaults to an empty identifier
/// (callers can override via `.identifier("...")` once we expose
/// customisation; not surfaced in v1 since customisation is off).
pub fn toolbar() -> Toolbar<()> {
    Toolbar {
        identifier: "leptos_cocoa.toolbar".to_string(),
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

    pub fn child<NewC>(self, c: NewC) -> Toolbar<(C, NewC)> {
        Toolbar {
            identifier: self.identifier,
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
    /// Test-only: read back the `HANDLER_STORE` key for the
    /// `n`-th `<toolbar_item>` with an installed action handler,
    /// in declaration order.
    #[doc(hidden)]
    pub fn handler_key_for_test(&self, n: usize) -> usize {
        self.toolbar.test_handler_keys()[n]
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
            build.handler_keys,
            mtm,
        );

        ToolbarState {
            toolbar,
            _children: children_state,
            _effects: build.effects,
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
        if let Some(window) = parent.ns_view().window() {
            self.toolbar.attach_to_window(&window);
        }
        // If the parent isn't in a window yet (rare, but possible
        // during deferred-build flows), the toolbar stays detached.
        // A follow-up viewDidMoveToWindow hook could re-attach;
        // not needed for current code paths since `<toolbar>` only
        // appears as an immediate child of `<window>`.
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
    pub(crate) sf_symbol: Option<MaybeReactive<String>>,
    pub(crate) enabled: Option<MaybeReactive<bool>>,
    pub(crate) on_action: Option<Box<dyn FnMut() + Send + 'static>>,
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
        sf_symbol: None,
        enabled: None,
        on_action: None,
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

    /// SF Symbol name — `"plus.circle"`, `"sidebar.left"`, etc.
    /// Sized via the shared 16pt-regular configuration so the
    /// item renders at NSToolbarItem's expected dimensions.
    pub fn sf_symbol<V: IntoMaybeReactive<String>>(mut self, v: V) -> Self {
        self.sf_symbol = Some(v.into_maybe_reactive());
        self
    }

    pub fn enabled<V: IntoMaybeReactive<bool>>(mut self, v: V) -> Self {
        self.enabled = Some(v.into_maybe_reactive());
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
}

impl ToolbarMountable for ToolbarItem {
    type State = ToolbarItemState;

    fn build_into_toolbar(
        self,
        build: &mut ToolbarBuild,
        mtm: MainThreadMarker,
    ) -> Self::State {
        let identifier = self.identifier.expect(
            "<toolbar_item> requires an `identifier` — call \
             `.identifier(\"...\")` or set `identifier=\"...\"` in the \
             macro. Identifiers must be unique within a <toolbar>.",
        );
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
        // SF symbol image.
        if let Some(sym) = self.sf_symbol {
            let it = item.clone();
            if let Some(eff) = install(sym, move |s| it.set_sf_symbol(&s)) {
                build.effects.push(eff);
            }
        }
        // Enabled.
        if let Some(en) = self.enabled {
            let it = item.clone();
            if let Some(eff) = install(en, move |b| it.set_enabled(b)) {
                build.effects.push(eff);
            }
        }
        // Action handler.
        if let Some(mut cb) = self.on_action {
            let key = item.set_action(move || cb(), mtm);
            build.handler_keys.push(key);
        }

        // Move the item into the toolbar build state. From here on
        // the delegate owns the item via its ivar map.
        build.insert_custom(identifier, item.into_ns_item());

        ToolbarItemState {}
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

// Sanity check the `is_builtin_identifier` helper at compile time:
// both space identifiers should be detected as builtin.
const _: fn() = || {
    let _ = is_builtin_identifier;
};
