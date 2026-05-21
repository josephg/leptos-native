//! `view!{}`-compatible menu builders: `<menu_bar>`, `<menu>`,
//! `<menu_item>`, `<menu_separator/>`.
//!
//! Native menus live *off* the Taffy layout tree (they're not
//! NSViews), so they have their own port-local mount cascade —
//! [`MenuMountable`] — that parallels [`Mountable<Dom>`] but threads
//! a [`MenuParent`] (either an `NSMenu` main-menu or a submenu)
//! through the build.
//!
//! Three builder types:
//!
//! - [`MenuBar<C>`] — top-level container. The only menu builder
//!   that implements [`Render<Dom>`]; siblings to `<window>` in
//!   `run()` see it as just another renderable. Build-time:
//!   constructs a fresh `cocoa_dom::menu::MenuBar`, descends into
//!   children, then `setMainMenu:`s it on NSApp.
//! - [`Menu<C>`] — submenu. Lives only as a child of [`MenuBar`]
//!   or another [`Menu`] (compile error elsewhere — we don't
//!   impl `Render<Dom>` for it).
//! - [`MenuItem`] / [`MenuSeparator`] — leaves. Same compile-error
//!   shape as `Menu`: outside `<menu>` they don't satisfy
//!   `Render<Dom>` so they fail at the call site, not at runtime.
//!
//! Tuple / `Option` / `Vec` impls of `MenuMountable` propagate the
//! cascade through the macro's flat-tuple grouping
//! (`((), (m0, m1, …))`) and through `<For>`/`<Show>` outputs.

use crate::cocoa::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::event_macos::{
    ActionEvent, EventDescriptor, PendingHandler, SupportsEvent,
};
use crate::Dom;
use cocoa_dom::{
    menu::{self as dom_menu, MenuBar as DomMenuBar},
    Element as CocoaElement, MainThreadMarker, CocoaNode as CocoaNode,
};
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSMenuItem};
use objc2_foundation::NSString;
use reactive_graph::effect::RenderEffect;
use renderer::menu::Modifiers;
use renderer::view::{Mountable, Render};

// ---------------------------------------------------------------------
// MenuParent + MenuMountable
// ---------------------------------------------------------------------

/// What you're attaching a child to: either the menu bar's top
/// level (one wrapper-item per submenu) or an `NSMenu` proper (a
/// vertical list of items + nested submenus).
///
/// Borrowed during the build cascade; never stored on the heap.
pub enum MenuParent<'a> {
    Bar(&'a DomMenuBar),
    Menu(&'a dom_menu::Menu),
}

/// Port-local analogue of [`Mountable<Dom>`] for the menu tree.
/// Implemented by every menu builder + the wrapper types the macro
/// produces (`()`, tuples, `Option`, `Vec`).
///
/// Stays port-local because cocoa's parent shape (NSMenu*) is
/// nothing like GTK's (`gio::Menu`). The trait will be mirrored —
/// not shared — on the GTK side.
pub trait MenuMountable: Send + 'static {
    /// State retained for the lifetime of the menu (so reactive
    /// effects and ActionTargets aren't dropped).
    type State: 'static;

    /// Build the item(s) and attach to `parent`. May install
    /// reactive effects against the underlying menu/item handle.
    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State;
}

// () — terminator for the children-tuple cascade.
impl MenuMountable for () {
    type State = ();
    fn build_into_menu(
        self,
        _parent: &MenuParent,
        _mtm: MainThreadMarker,
    ) -> Self::State {
    }
}

// (A, B) — flat tuples produced by the macro for >1 child.
macro_rules! impl_tuple {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<$($T),+> MenuMountable for ($($T,)+)
        where
            $($T: MenuMountable,)+
        {
            type State = ($($T::State,)+);
            fn build_into_menu(
                self,
                parent: &MenuParent,
                mtm: MainThreadMarker,
            ) -> Self::State {
                ( $( self.$idx.build_into_menu(parent, mtm), )+ )
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

// Option — for `<Show>`-like conditional rendering.
impl<T: MenuMountable> MenuMountable for Option<T> {
    type State = Option<T::State>;
    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State {
        self.map(|t| t.build_into_menu(parent, mtm))
    }
}

// Vec — for `<For>`-driven item lists.
impl<T: MenuMountable> MenuMountable for Vec<T> {
    type State = Vec<T::State>;
    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State {
        self.into_iter()
            .map(|t| t.build_into_menu(parent, mtm))
            .collect()
    }
}

// ---------------------------------------------------------------------
// MenuBar — top-level container, the only menu builder that's
// Render<Dom>
// ---------------------------------------------------------------------

/// Top-level menu container. Sits as a sibling of `<window>` in
/// `run()`'s root tuple. Use [`menu_bar`] to construct.
pub struct MenuBar<Children> {
    pub(crate) children: Children,
}

/// Construct an empty `<menu_bar>`. Add submenus via `.child(...)`
/// or by writing `<menu>`s inside the `view!{}` form.
pub fn menu_bar() -> MenuBar<()> {
    MenuBar { children: () }
}

impl<C> MenuBar<C> {
    pub fn child<NewC>(self, c: NewC) -> MenuBar<(C, NewC)> {
        MenuBar {
            children: (self.children, c),
        }
    }
}

#[doc(hidden)]
pub struct MenuBarState<CS> {
    _bar: DomMenuBar,
    _children: CS,
}

impl<C> Render<Dom> for MenuBar<C>
where
    C: MenuMountable,
{
    type State = MenuBarState<C::State>;

    fn build(self) -> Self::State {
        let mtm = MainThreadMarker::new()
            .expect("MenuBar::build must run on the main thread");
        let bar = dom_menu::menu_bar(mtm);

        // Descend into children first so the bar is fully populated
        // before we install it on NSApp. Otherwise menu-bar paint
        // would briefly flash an empty bar.
        let children_state =
            self.children.build_into_menu(&MenuParent::Bar(&bar), mtm);

        // Install onto NSApp.mainMenu — overwrites whatever
        // `init_app` previously set up (the App + Edit baseline).
        // Per the design, v1 replaces rather than extends.
        let app = NSApplication::sharedApplication(mtm);
        bar.install(&app);

        MenuBarState {
            _bar: bar,
            _children: children_state,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {
        // Reactive titles/enabled/checked update via their own
        // effects. Structural rebuild not supported yet.
    }
}

impl<CS: 'static> Mountable<Dom> for MenuBarState<CS> {
    fn unmount(&mut self) {
        // No-op for now: NSApp.mainMenu sticks around for the
        // process lifetime by convention, and an explicit clear
        // would race with the AppKit run loop's menu-tracking.
    }
    fn mount(
        &mut self,
        _parent: &CocoaElement,
        _marker: Option<&CocoaNode>,
    ) {
        // The menu bar is its own root; nothing to attach under
        // a view-tree parent.
    }
    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }
    fn elements(&self) -> Vec<CocoaElement> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------
// Menu — submenu (NSMenu attached to a parent menu/bar item)
// ---------------------------------------------------------------------

/// One submenu: an `NSMenu` exposed in its parent as a single
/// `NSMenuItem` whose `submenu:` is the underlying menu. Build via
/// [`menu`].
pub struct Menu<Children> {
    /// `None` until [`Menu::title`] is called. Missing-title at
    /// build time is a panic, matching the plan's
    /// "fail-loud" rule for a `<menu>` with no label (which would
    /// render as a blank menu-bar item).
    pub(crate) title:    Option<MaybeReactive<String>>,
    pub(crate) children: Children,
}

/// Start configuring a submenu. Title is required — set via
/// [`Menu::title`] before mounting.
pub fn menu() -> Menu<()> {
    Menu { title: None, children: () }
}

impl<C> Menu<C> {
    pub fn title<V>(mut self, t: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = Some(t.into_maybe_reactive());
        self
    }

    pub fn child<NewC>(self, c: NewC) -> Menu<(C, NewC)> {
        Menu {
            title:    self.title,
            children: (self.children, c),
        }
    }
}

#[doc(hidden)]
pub struct MenuState<CS> {
    _menu:         dom_menu::Menu,
    _wrapper_item: Retained<NSMenuItem>,
    _children:     CS,
    _effects:      Vec<RenderEffect<()>>,
}

impl<C> MenuMountable for Menu<C>
where
    C: MenuMountable,
{
    type State = MenuState<C::State>;

    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State {
        // Build the underlying NSMenu. The title starts blank; the
        // reactive setter below pushes the real value (and keeps
        // updating it on signal changes).
        let dom_menu_handle = dom_menu::menu("", mtm);

        // Attach to parent and grab the wrapper item so we can
        // update its title independently — AppKit reads the
        // wrapper's title for menu-bar display, not the submenu's.
        let wrapper_item = match parent {
            MenuParent::Bar(bar) => bar.append_menu(&dom_menu_handle, mtm),
            MenuParent::Menu(m) => m.append_submenu(&dom_menu_handle, mtm),
        };

        let title = self.title.expect(
            "<menu> requires a title — call `.title(\"…\")` (or set \
             `title=\"…\"` in the macro). A blank submenu renders as \
             an unreachable menu-bar item.",
        );

        // Reactive title: update both the menu (used when the menu
        // is reached via the keyboard / NSMenu API) and the
        // wrapper item (used for the menu-bar / parent-menu label —
        // AppKit copies the submenu's title to the wrapper at
        // attach time but doesn't keep them in sync afterward).
        let mut effects = Vec::new();
        let menu_for = dom_menu_handle.clone();
        let wrapper_for = wrapper_item.clone();
        if let Some(eff) = install(title, move |t| {
            menu_for.set_title(&t);
            wrapper_for.setTitle(&NSString::from_str(&t));
        }) {
            effects.push(eff);
        }

        // Descend into the submenu's children.
        let children_state = self.children.build_into_menu(
            &MenuParent::Menu(&dom_menu_handle),
            mtm,
        );

        MenuState {
            _menu:         dom_menu_handle,
            _wrapper_item: wrapper_item,
            _children:     children_state,
            _effects:      effects,
        }
    }
}

// ---------------------------------------------------------------------
// MenuItem — leaf command
// ---------------------------------------------------------------------

/// Leaf menu command. Title, enabled, checked, shortcut, and a
/// single `on:action` handler. Build via [`menu_item`].
pub struct MenuItem {
    /// `None` until `.title(...)` is called — missing-title items
    /// panic at build time (a blank-strip render is rarely the
    /// intent).
    pub(crate) title:    Option<MaybeReactive<String>>,
    pub(crate) enabled:  Option<MaybeReactive<bool>>,
    pub(crate) checked:  Option<MaybeReactive<bool>>,
    pub(crate) shortcut_key:       Option<String>,
    pub(crate) shortcut_modifiers: Option<Modifiers>,
    /// Icon shown in the menu-item icon column. SF Symbol or
    /// file path — see [`cocoa_dom::Icon`]. Reactive.
    pub(crate) icon: Option<MaybeReactive<cocoa_dom::Icon>>,
    /// Single `on:action` slot. `None` is valid (informational
    /// item). Second `.on(event::action, …)` call panics — see the
    /// `on()` method.
    pub(crate) on_action: Option<Box<dyn FnMut() + Send + 'static>>,
}

/// Start configuring a leaf menu item. Title is required — set via
/// `.title(...)` before mounting.
pub fn menu_item() -> MenuItem {
    MenuItem {
        title:    None,
        enabled:  None,
        checked:  None,
        shortcut_key:       None,
        shortcut_modifiers: None,
        icon:     None,
        on_action: None,
    }
}

impl MenuItem {
    pub fn title<V>(mut self, t: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = Some(t.into_maybe_reactive());
        self
    }

    pub fn enabled<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(b.into_maybe_reactive());
        self
    }

    /// Show a check-mark when this is `true`. Reactive — flip the
    /// signal and AppKit will tick / untick the item.
    pub fn checked<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.checked = Some(b.into_maybe_reactive());
        self
    }

    /// Icon shown in the menu item's icon column. Pass an
    /// [`cocoa_dom::Icon`] directly
    /// (`Icon::sf_symbol("doc.badge.plus")` or
    /// `Icon::image("/path/to/file.png")`), or a reactive closure
    /// returning one.
    pub fn icon<V>(mut self, v: V) -> Self
    where
        V: IntoMaybeReactive<cocoa_dom::Icon>,
    {
        self.icon = Some(v.into_maybe_reactive());
        self
    }

    /// Keyboard-equivalent character (e.g. `"r"` for the "R" key,
    /// `""` to clear). Static — reactive shortcuts aren't useful
    /// in practice and add bookkeeping cost.
    ///
    /// When set without an accompanying [`modifiers`](Self::modifiers)
    /// call, defaults to [`Modifiers::CMD`].
    pub fn shortcut(mut self, key: impl Into<String>) -> Self {
        self.shortcut_key = Some(key.into());
        self
    }

    /// Override the modifier flags for the shortcut. Only
    /// meaningful when [`shortcut`](Self::shortcut) is also set.
    pub fn modifiers(mut self, m: Modifiers) -> Self {
        self.shortcut_modifiers = Some(m);
        self
    }

    /// Inline `on:event=handler` from the macro. Only `on:action`
    /// makes sense on a menu item; the [`SupportsEvent`] bound
    /// rejects others at compile time. A second `.on(event::action,
    /// _)` panics — single-handler contract mirrors NSControl's
    /// target/action slot.
    #[track_caller]
    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        // Lower the typed handler through PendingHandler so we
        // share the `E::into_pending` plumbing with normal events.
        // Only `Action` reaches here per the SupportsEvent bound.
        match E::into_pending(handler) {
            PendingHandler::Action(cb) => {
                if self.on_action.is_some() {
                    panic!(
                        "<menu_item> already has an on:action handler. \
                         NSMenuItem has a single target/action slot; \
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

// Compile-time control/event compatibility — `<menu_item on:click>`
// won't compile.
impl SupportsEvent<ActionEvent> for MenuItem {}

#[doc(hidden)]
pub struct MenuItemState {
    _item:    dom_menu::MenuItem,
    _effects: Vec<RenderEffect<()>>,
}

// No `Drop` impl: the ActionTarget is attached as an associated
// object on the NSMenuItem, so the ObjC runtime releases it when
// the menu item itself deallocates. Releasing happens whenever the
// menu bar (or sub-menu containing this item) drops its retain.

impl MenuMountable for MenuItem {
    type State = MenuItemState;

    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State {
        let title = self.title.expect(
            "<menu_item> requires a title — call `.title(\"…\")` (or set \
             `title=\"…\"` in the macro).",
        );

        let item = dom_menu::menu_item(mtm);
        let mut effects = Vec::new();

        // Title.
        let it = item.clone();
        if let Some(eff) = install(title, move |t| it.set_title(&t)) {
            effects.push(eff);
        }
        // Enabled.
        if let Some(e) = self.enabled {
            let it = item.clone();
            if let Some(eff) = install(e, move |b| it.set_enabled(b)) {
                effects.push(eff);
            }
        }
        // Checked.
        if let Some(c) = self.checked {
            let it = item.clone();
            if let Some(eff) = install(c, move |b| it.set_checked(b)) {
                effects.push(eff);
            }
        }
        // Icon (SF Symbol or file path, unified).
        if let Some(ic) = self.icon {
            let it = item.clone();
            if let Some(eff) = install(ic, move |icon| {
                it.set_icon(Some(&icon));
            }) {
                effects.push(eff);
            }
        }
        // Shortcut (static).
        if let Some(key) = self.shortcut_key {
            let mods = self.shortcut_modifiers.unwrap_or(Modifiers::CMD);
            item.set_shortcut(&key, mods);
        }

        // Action handler — single one, enforced by the builder
        // (`MenuItem::on` panics on second install).
        if let Some(mut cb) = self.on_action {
            item.set_action(move || cb(), mtm);
        }

        // Attach. <menu_item> directly under <menu_bar> is invalid
        // — wrap in a <menu title=…> first.
        match parent {
            MenuParent::Menu(m) => m.append_item(&item),
            MenuParent::Bar(_) => panic!(
                "<menu_item> must be a child of <menu>, not directly \
                 under <menu_bar>. Wrap your items in <menu title=\"…\"> \
                 first."
            ),
        }

        MenuItemState {
            _item:    item,
            _effects: effects,
        }
    }
}

// ---------------------------------------------------------------------
// MenuSeparator — divider
// ---------------------------------------------------------------------

/// Horizontal divider between groups of related items. No
/// configuration — use [`menu_separator`].
pub struct MenuSeparator;

/// Construct a menu separator (`+[NSMenuItem separatorItem]`).
pub fn menu_separator() -> MenuSeparator {
    MenuSeparator
}

#[doc(hidden)]
pub struct MenuSeparatorState {
    _item: dom_menu::MenuItem,
}

impl MenuMountable for MenuSeparator {
    type State = MenuSeparatorState;

    fn build_into_menu(
        self,
        parent: &MenuParent,
        mtm: MainThreadMarker,
    ) -> Self::State {
        let item = dom_menu::menu_separator(mtm);
        match parent {
            MenuParent::Menu(m) => m.append_item(&item),
            MenuParent::Bar(_) => panic!(
                "<menu_separator/> must be a child of <menu>, not \
                 directly under <menu_bar>."
            ),
        }
        MenuSeparatorState { _item: item }
    }
}
