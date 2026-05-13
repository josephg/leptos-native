//! `view!{}`-compatible menu builders for the GTK port. Mirrors
//! `leptos_cocoa::cocoa::menu` — same surface, same port-local
//! [`MenuMountable`] cascade, different underlying model.
//!
//! GTK menus live entirely off the widget tree: `gio::Menu` is a
//! pure data model that the application installs via
//! `gtk_application_set_menubar`. The application's compositor
//! (Cinnamon menubar, GNOME unified menubar, etc.) is responsible
//! for displaying it. Action items fire via the
//! `gio::Action` group on the application — each `<menu_item>`
//! gets its own auto-generated `app.menuitem_N` action.
//!
//! Reactive titles: `gio::MenuItem` is immutable once added to a
//! `gio::Menu`, so the reactive setter for `title=move || …`
//! evicts and re-inserts the item at the same index. The bookkeeping
//! lives in [`MenuItemState`] (the parent menu + insertion index).

use crate::event_gtk::{
    ActionEvent, EventDescriptor, PendingHandler, SupportsEvent,
};
use crate::gtk::attr::{install, IntoMaybeReactive, MaybeReactive};
use crate::Dom;
use gtk4::prelude::*;
use gtk_dom::menu::{self as dom_menu, MenuBar as DomMenuBar};
use reactive_graph::effect::RenderEffect;
use renderer::menu::Modifiers;
use renderer::view::{Mountable, Render};
use std::cell::Cell;
use std::rc::Rc;

// ---------------------------------------------------------------------
// MenuParent + MenuMountable
// ---------------------------------------------------------------------

/// What we're attaching a child to. Bar = the application's top-
/// level `gio::Menu` (rendered as the menu bar); Menu = a submenu.
///
/// Both variants carry the `gtk::Application` because reactive
/// menu-item-title updates and accel bindings need it.
pub enum MenuParent<'a> {
    Bar(&'a DomMenuBar),
    Menu {
        menu: &'a dom_menu::Menu,
        app:  &'a gtk4::Application,
    },
}

impl<'a> MenuParent<'a> {
    fn app(&self) -> &gtk4::Application {
        match self {
            MenuParent::Bar(bar) => bar.app(),
            MenuParent::Menu { app, .. } => app,
        }
    }
}

/// Port-local analogue of [`Mountable<Dom>`] for the menu tree.
/// Implemented by every menu builder + the wrapper types the macro
/// produces (`()`, tuples, `Option`, `Vec`).
pub trait MenuMountable: Send + 'static {
    type State: 'static;
    fn build_into_menu(self, parent: &MenuParent) -> Self::State;
}

impl MenuMountable for () {
    type State = ();
    fn build_into_menu(self, _parent: &MenuParent) -> Self::State {}
}

macro_rules! impl_tuple {
    ($(($idx:tt, $T:ident)),+ $(,)?) => {
        impl<$($T),+> MenuMountable for ($($T,)+)
        where
            $($T: MenuMountable,)+
        {
            type State = ($($T::State,)+);
            fn build_into_menu(self, parent: &MenuParent) -> Self::State {
                ( $( self.$idx.build_into_menu(parent), )+ )
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

impl<T: MenuMountable> MenuMountable for Option<T> {
    type State = Option<T::State>;
    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        self.map(|t| t.build_into_menu(parent))
    }
}

impl<T: MenuMountable> MenuMountable for Vec<T> {
    type State = Vec<T::State>;
    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        self.into_iter().map(|t| t.build_into_menu(parent)).collect()
    }
}

// ---------------------------------------------------------------------
// MenuBar — top-level container, the only menu builder that's
// Render<Dom>
// ---------------------------------------------------------------------

pub struct MenuBar<Children> {
    pub(crate) children: Children,
}

/// Construct an empty `<menu_bar>`. Lookups its
/// `gtk::Application` via `gio::Application::default()` at build
/// time — works inside `run()`'s activate handler, panics otherwise.
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
        // GTK doesn't have a process-wide singleton like NSApp.
        // The app is set as default by gtk::Application::new
        // (which init_app calls); inside the activate handler
        // gio::Application::default() returns it. Downcast to the
        // gtk::Application subclass.
        let app: gtk4::Application = gio::Application::default()
            .expect(
                "MenuBar::build called with no default gio::Application — \
                 use leptos::run() to set one up before building views.",
            )
            .downcast::<gtk4::Application>()
            .expect(
                "default gio::Application is not a gtk::Application — \
                 this should be impossible inside leptos::run().",
            );

        let bar = dom_menu::menu_bar(&app);

        // Build children before installing — analogous to cocoa.
        let children_state =
            self.children.build_into_menu(&MenuParent::Bar(&bar));

        bar.install();

        MenuBarState {
            _bar: bar,
            _children: children_state,
        }
    }

    fn rebuild(self, _state: &mut Self::State) {}
}

impl<CS: 'static> Mountable<Dom> for MenuBarState<CS> {
    fn unmount(&mut self) {}
    fn mount(
        &mut self,
        _parent: &gtk_dom::Element,
        _marker: Option<&gtk_dom::Node>,
    ) {
    }
    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }
    fn elements(&self) -> Vec<gtk_dom::Element> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------
// Menu — submenu
// ---------------------------------------------------------------------

pub struct Menu<Children> {
    pub(crate) title:    MaybeReactive<String>,
    pub(crate) children: Children,
}

pub fn menu() -> Menu<()> {
    Menu {
        title:    MaybeReactive::Static(String::new()),
        children: (),
    }
}

impl<C> Menu<C> {
    pub fn title<V>(mut self, t: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = t.into_maybe_reactive();
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
    _menu:     dom_menu::Menu,
    _children: CS,
    _effects:  Vec<RenderEffect<()>>,
}

impl<C> MenuMountable for Menu<C>
where
    C: MenuMountable,
{
    type State = MenuState<C::State>;

    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        let sub = dom_menu::menu();
        let app = parent.app().clone();

        // Append immediately with a placeholder title; the reactive
        // setter then drives both initial value and subsequent
        // updates (re-inserting the wrapper item at the same
        // index).
        let idx = match parent {
            MenuParent::Bar(bar) => bar.append_submenu("", &sub),
            MenuParent::Menu { menu, .. } => menu.append_submenu("", &sub),
        };

        // Reactive title — `gio::MenuItem` is immutable, so reactive
        // updates remove + re-insert. The Rc<Cell<usize>> holds the
        // current index in case future insertions shift it (not yet
        // wired; v1 assumes the menu structure is stable after
        // build).
        let idx_cell = Rc::new(Cell::new(idx));
        let mut effects = Vec::new();
        let sub_for = sub.clone();
        let parent_handle = parent_replace_handle(parent);
        let app_for = app.clone();
        let _ = app_for; // suppress unused if no reactive title is set
        if let Some(eff) = install(self.title, move |t| {
            let i = idx_cell.get();
            parent_handle.replace_submenu(i, &t, &sub_for);
        }) {
            effects.push(eff);
        }

        // Descend into children.
        let new_parent = MenuParent::Menu { menu: &sub, app: &app };
        let children_state = self.children.build_into_menu(&new_parent);

        MenuState {
            _menu:     sub,
            _children: children_state,
            _effects:  effects,
        }
    }
}

/// Owned handle for the menu's parent — captured into the reactive
/// title closure (which outlives the borrowed [`MenuParent`]).
/// Both variants are cheap to clone: gio glib objects are
/// refcounted.
enum ParentReplaceHandle {
    Bar(DomMenuBar),
    Menu(dom_menu::Menu),
}

impl ParentReplaceHandle {
    fn replace_submenu(&self, idx: usize, label: &str, sub: &dom_menu::Menu) {
        match self {
            ParentReplaceHandle::Bar(b) => b.replace_submenu(idx, label, sub),
            ParentReplaceHandle::Menu(m) => m.replace_submenu(idx, label, sub),
        }
    }
}

fn parent_replace_handle(parent: &MenuParent) -> ParentReplaceHandle {
    match parent {
        MenuParent::Bar(b) => ParentReplaceHandle::Bar((*b).clone()),
        MenuParent::Menu { menu, .. } => ParentReplaceHandle::Menu((*menu).clone()),
    }
}

// ---------------------------------------------------------------------
// MenuItem — leaf command
// ---------------------------------------------------------------------

pub struct MenuItem {
    pub(crate) title:    MaybeReactive<String>,
    pub(crate) enabled:  Option<MaybeReactive<bool>>,
    pub(crate) checked:  Option<MaybeReactive<bool>>,
    pub(crate) shortcut_key:       Option<String>,
    pub(crate) shortcut_modifiers: Option<Modifiers>,
    pub(crate) handlers: Vec<PendingHandler>,
}

pub fn menu_item() -> MenuItem {
    MenuItem {
        title:    MaybeReactive::Static(String::new()),
        enabled:  None,
        checked:  None,
        shortcut_key:       None,
        shortcut_modifiers: None,
        handlers: Vec::new(),
    }
}

impl MenuItem {
    pub fn title<V>(mut self, t: V) -> Self
    where
        V: IntoMaybeReactive<String>,
    {
        self.title = t.into_maybe_reactive();
        self
    }

    pub fn enabled<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.enabled = Some(b.into_maybe_reactive());
        self
    }

    pub fn checked<V>(mut self, b: V) -> Self
    where
        V: IntoMaybeReactive<bool>,
    {
        self.checked = Some(b.into_maybe_reactive());
        self
    }

    pub fn shortcut(mut self, key: impl Into<String>) -> Self {
        self.shortcut_key = Some(key.into());
        self
    }

    pub fn modifiers(mut self, m: Modifiers) -> Self {
        self.shortcut_modifiers = Some(m);
        self
    }

    pub fn on<E, F>(mut self, _event: E, handler: F) -> Self
    where
        Self: SupportsEvent<E>,
        E: EventDescriptor,
        F: FnMut(E::EventType) + Send + 'static,
    {
        self.handlers.push(E::into_pending(handler));
        self
    }
}

impl SupportsEvent<ActionEvent> for MenuItem {}

#[doc(hidden)]
pub struct MenuItemState {
    _item:    dom_menu::MenuItem,
    _effects: Vec<RenderEffect<()>>,
}

impl MenuMountable for MenuItem {
    type State = MenuItemState;

    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        let item = dom_menu::menu_item();
        let app = parent.app().clone();
        let mut effects = Vec::new();

        // Wire action *before* attaching so the gio::Action exists
        // on the application by the time the menu is rendered.
        for h in self.handlers {
            match h {
                PendingHandler::Action(mut cb) => {
                    item.set_action(&app, move || cb());
                }
                _ => panic!(
                    "<menu_item> only supports on:action handlers."
                ),
            }
        }

        // Append to parent. <menu_item> directly under <menu_bar>
        // panics — wrap in <menu> first.
        let parent_menu = match parent {
            MenuParent::Menu { menu, .. } => menu,
            MenuParent::Bar(_) => panic!(
                "<menu_item> must be a child of <menu>, not directly \
                 under <menu_bar>. Wrap your items in <menu title=\"…\"> \
                 first."
            ),
        };
        let idx = parent_menu.append_item(&item);

        // Title is reactive — gio::MenuItem is immutable so we
        // evict and re-insert. The set_label path on the
        // already-detached MenuItem keeps our local handle in sync
        // for future reactive runs.
        let parent_for: dom_menu::Menu = (*parent_menu).clone();
        let item_for = item.clone();
        let idx_cell = Rc::new(Cell::new(idx));
        if let Some(eff) = install(self.title, move |t| {
            item_for.set_label(&t);
            parent_for.replace_item(idx_cell.get(), &item_for);
        }) {
            effects.push(eff);
        }

        // Enabled.
        if let Some(e) = self.enabled {
            let it = item.clone();
            if let Some(eff) = install(e, move |b| it.set_enabled(b)) {
                effects.push(eff);
            }
        }
        // Checked (v1 stub on GTK — see gtk_dom::menu::MenuItem).
        if let Some(c) = self.checked {
            let it = item.clone();
            if let Some(eff) = install(c, move |b| it.set_checked(b)) {
                effects.push(eff);
            }
        }
        // Shortcut.
        if let Some(key) = self.shortcut_key {
            let mods = self.shortcut_modifiers.unwrap_or(Modifiers::CMD);
            item.set_shortcut(&app, &key, mods);
        }

        MenuItemState {
            _item:    item,
            _effects: effects,
        }
    }
}

// ---------------------------------------------------------------------
// MenuSeparator
// ---------------------------------------------------------------------

pub struct MenuSeparator;

pub fn menu_separator() -> MenuSeparator {
    MenuSeparator
}

#[doc(hidden)]
pub struct MenuSeparatorState {
    _section: dom_menu::Menu,
}

impl MenuMountable for MenuSeparator {
    type State = MenuSeparatorState;

    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        // GTK groups items into sections; adjacent sections render
        // with a divider between them. The simplest "separator" is
        // an empty section: a fresh `gio::Menu` appended via
        // `append_section`. Items added to the same parent menu
        // after this will appear in a new visual group.
        let section = dom_menu::menu();
        match parent {
            MenuParent::Menu { menu, .. } => {
                menu.append_section(&section);
            }
            MenuParent::Bar(_) => panic!(
                "<menu_separator/> must be a child of <menu>, not \
                 directly under <menu_bar>."
            ),
        }
        MenuSeparatorState { _section: section }
    }
}
