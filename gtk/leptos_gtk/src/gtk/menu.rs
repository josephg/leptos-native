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
use crate::dom::menu::{self as dom_menu, MenuBar as DomMenuBar};
use reactive_graph::effect::RenderEffect;
use renderer::menu::Modifiers;
use renderer::view::{Mountable, Render};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use crate::dom::GtkNode;
// ---------------------------------------------------------------------
// SectionCursor — gio-section-aware grouping for <menu_separator/>
// ---------------------------------------------------------------------
//
// gio menus model "divider between groups of items" as separate
// `gio::Menu` sections appended to a parent via `append_section`. The
// renderer draws a divider line between adjacent sections, but does
// *not* draw a divider between two loose items in the same section.
//
// So to honour `<menu_separator/>`, each <menu>'s items have to be
// grouped into sections at build time, with each separator
// terminating the current section and opening a new one. The cursor
// here is the shared mutable state that lets MenuItem /
// MenuSeparator / nested Menu builds all push into the right
// section.

/// Per-`<menu>` section accumulator. Created when entering a
/// submenu's build, flushed at exit so the final section makes it
/// into the parent menu.
pub struct SectionCursor {
    /// The menu that finished sections get appended to.
    parent: dom_menu::Menu,
    /// The currently-open section, or `None` if no items have been
    /// appended since the last separator (or build start). Lazily
    /// allocated.
    open:   RefCell<Option<dom_menu::Menu>>,
}

impl SectionCursor {
    fn new(parent: dom_menu::Menu) -> Self {
        Self { parent, open: RefCell::new(None) }
    }

    /// Ensure a section is open and return a handle. Children of
    /// the current group append to this section.
    fn ensure_open(&self) -> dom_menu::Menu {
        let mut open = self.open.borrow_mut();
        if open.is_none() {
            *open = Some(dom_menu::menu());
        }
        open.as_ref().unwrap().clone()
    }

    /// Close the currently-open section (if any) by appending it to
    /// the parent. The next item appended via [`Self::ensure_open`]
    /// starts a fresh section.
    fn flush(&self) {
        let mut open = self.open.borrow_mut();
        if let Some(section) = open.take() {
            self.parent.append_section(&section);
        }
    }
}

// ---------------------------------------------------------------------
// MenuParent + MenuMountable
// ---------------------------------------------------------------------

/// What we're attaching a child to. Bar = the application's top-
/// level `gio::Menu` (rendered as the menu bar); Menu = a submenu
/// with a section cursor for separator-driven grouping.
///
/// Both variants carry the `gtk::Application` because reactive
/// menu-item-title updates and accel bindings need it.
pub enum MenuParent<'a> {
    Bar(&'a DomMenuBar),
    Menu {
        app:    &'a gtk4::Application,
        /// Section accumulator — items go into the currently-open
        /// section (lazily allocated). `<menu_separator/>` flushes
        /// it so the next item starts a fresh section.
        cursor: &'a SectionCursor,
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
                 use leptos_native::run() to set one up before building views.",
            )
            .downcast::<gtk4::Application>()
            .expect(
                "default gio::Application is not a gtk::Application — \
                 this should be impossible inside leptos_native::run().",
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
}

impl<CS: 'static> Mountable<Dom> for MenuBarState<CS> {
    fn unmount(&mut self) {}
    fn mount(
        &mut self,
        _parent: &GtkNode,
        _marker: Option<&GtkNode>,
    ) {
    }
    fn insert_before_this(&self, _child: &mut dyn Mountable<Dom>) -> bool {
        false
    }
    fn elements(&self) -> Vec<GtkNode> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------
// Menu — submenu
// ---------------------------------------------------------------------

pub struct Menu<Children> {
    /// `None` until [`Menu::title`] is called. Missing title at
    /// build time is a panic — matches the cocoa fail-loud rule.
    pub(crate) title:    Option<MaybeReactive<String>>,
    pub(crate) children: Children,
}

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
        let title = self.title.expect(
            "<menu> requires a title — call `.title(\"…\")` (or set \
             `title=\"…\"` in the macro).",
        );

        let sub = dom_menu::menu();
        let app = parent.app().clone();

        // Append immediately with a placeholder title; `install`
        // runs the closure synchronously for the initial value, so
        // the empty wrapper exists only for one synchronous call
        // before being replaced with the real title.
        //
        // Under the section-cursor model, a nested <menu> wrapper
        // item lives inside its parent's currently-open section
        // (not in the parent menu directly). The MenuBar case has
        // no section cursor — wrapper items sit directly in the
        // menu bar's `gio::Menu`.
        let idx = match parent {
            MenuParent::Bar(bar) => bar.append_submenu("", &sub),
            MenuParent::Menu { cursor, .. } => {
                let section = cursor.ensure_open();
                section.append_submenu("", &sub)
            }
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
        if let Some(eff) = install(title, move |t| {
            let i = idx_cell.get();
            parent_handle.replace_submenu(i, &t, &sub_for);
        }) {
            effects.push(eff);
        }

        // Descend into children with a fresh section cursor scoped
        // to this submenu. The cursor's `flush` at the end of this
        // method seals the final section into `sub`.
        let cursor = SectionCursor::new(sub.clone());
        let new_parent = MenuParent::Menu { app: &app, cursor: &cursor };
        let children_state = self.children.build_into_menu(&new_parent);
        cursor.flush();

        MenuState {
            _menu:     sub,
            _children: children_state,
            _effects:  effects,
        }
    }
}

/// Owned handle for the menu's parent — captured into the reactive
/// title closure (which outlives the borrowed [`MenuParent`]). Both
/// variants are cheap to clone: gio glib objects are refcounted.
///
/// For the `Menu` arm we snapshot the section the wrapper item was
/// inserted into. Reactive title updates evict/re-insert the wrapper
/// at the same index *within that section*. The section's identity
/// is stable across the menu bar's lifetime — even after the section
/// has been flushed into the parent menu, it remains the live model
/// that the renderer reads, so future replace_item calls land in the
/// right spot.
enum ParentReplaceHandle {
    Bar(DomMenuBar),
    Section(dom_menu::Menu),
}

impl ParentReplaceHandle {
    fn replace_submenu(&self, idx: usize, label: &str, sub: &dom_menu::Menu) {
        match self {
            ParentReplaceHandle::Bar(b) => b.replace_submenu(idx, label, sub),
            ParentReplaceHandle::Section(s) => s.replace_submenu(idx, label, sub),
        }
    }
}

fn parent_replace_handle(parent: &MenuParent) -> ParentReplaceHandle {
    match parent {
        MenuParent::Bar(b) => ParentReplaceHandle::Bar((*b).clone()),
        MenuParent::Menu { cursor, .. } => {
            ParentReplaceHandle::Section(cursor.ensure_open())
        }
    }
}

// ---------------------------------------------------------------------
// MenuItem — leaf command
// ---------------------------------------------------------------------

pub struct MenuItem {
    /// `None` until `.title(...)` is called — missing-title items
    /// panic at build time.
    pub(crate) title:    Option<MaybeReactive<String>>,
    pub(crate) enabled:  Option<MaybeReactive<bool>>,
    pub(crate) checked:  Option<MaybeReactive<bool>>,
    pub(crate) shortcut_key:       Option<String>,
    pub(crate) shortcut_modifiers: Option<Modifiers>,
    /// Single `on:action` slot. `None` is valid (informational
    /// item). Second `.on(event::action, …)` call panics.
    pub(crate) on_action: Option<Box<dyn FnMut() + Send + 'static>>,
}

pub fn menu_item() -> MenuItem {
    MenuItem {
        title:    None,
        enabled:  None,
        checked:  None,
        shortcut_key:       None,
        shortcut_modifiers: None,
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

    /// Inline `on:event=handler` from the macro. Only `on:action`
    /// makes sense on a menu item (compile-time gated via
    /// [`SupportsEvent`]). A second call panics.
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
                        "<menu_item> already has an on:action handler. \
                         gio::SimpleAction has a single activate slot; \
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

impl SupportsEvent<ActionEvent> for MenuItem {}

#[doc(hidden)]
pub struct MenuItemState {
    /// Kept alive so the gio::MenuItem / gio::SimpleAction don't
    /// drop while the menu is still installed (the gio::Menu
    /// model only holds them by reference).
    _item:    dom_menu::MenuItem,
    /// Reactive-effect retains. Dropped before `_item` (struct
    /// fields drop in declaration order), so reactive setters
    /// stop firing before the item itself goes away.
    _effects: Vec<RenderEffect<()>>,
    /// The application this item registered its action on. Held
    /// so the [`Drop`] impl below can call `remove_action` and
    /// avoid leaking entries on the application's action group
    /// when the menu tree tears down.
    app:      gtk4::Application,
}

impl Drop for MenuItemState {
    fn drop(&mut self) {
        // Unregister the per-item `app.menuitem_N` action from the
        // application's action group. Otherwise every menu-bar
        // rebuild would accumulate dead actions on the app
        // (the auto-generated action name is process-wide unique,
        // so they'd never collide, but they'd never get GC'd
        // either). Cocoa's analog lives in
        // `cocoa_dom::menu::MenuItem::drop_handlers`.
        let _ = self.app.remove_action(self._item.action_name());
    }
}

impl MenuMountable for MenuItem {
    type State = MenuItemState;

    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        let title = self.title.expect(
            "<menu_item> requires a title — call `.title(\"…\")` (or set \
             `title=\"…\"` in the macro).",
        );

        // Pick the checkable variant up-front when the builder
        // has a `checked=` slot. GTK doesn't let us swap a plain
        // `SimpleAction` to a stateful one after the fact — the
        // underlying menu renderer only shows the check column for
        // items whose action carries state, so the decision lives
        // here at construction time. The reactive `checked` setter
        // below uses `set_state` against the now-stateful action.
        let item = if self.checked.is_some() {
            dom_menu::menu_item_checkable()
        } else {
            dom_menu::menu_item()
        };
        let app = parent.app().clone();
        let mut effects = Vec::new();

        // Wire action *before* attaching so the gio::Action exists
        // on the application by the time the menu is rendered.
        if let Some(mut cb) = self.on_action {
            item.set_action(&app, move || cb());
        }

        // Append into the parent's currently-open section.
        // <menu_item> directly under <menu_bar> panics — wrap in
        // <menu> first.
        let section = match parent {
            MenuParent::Menu { cursor, .. } => cursor.ensure_open(),
            MenuParent::Bar(_) => panic!(
                "<menu_item> must be a child of <menu>, not directly \
                 under <menu_bar>. Wrap your items in <menu title=\"…\"> \
                 first."
            ),
        };
        let idx = section.append_item(&item);

        // Title is reactive — gio::MenuItem is immutable so we
        // evict and re-insert *within the section*. The section
        // handle is stable across the menu's lifetime (it remains
        // the live model even after being flushed to its parent),
        // so future replace_item lands in the right spot.
        let parent_for: dom_menu::Menu = section;
        let item_for = item.clone();
        let idx_cell = Rc::new(Cell::new(idx));
        if let Some(eff) = install(title, move |t| {
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
        // Checked. The action is already stateful (set above), so
        // set_checked hits the underlying gio::SimpleAction's state.
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
            app,
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
pub struct MenuSeparatorState {}

impl MenuMountable for MenuSeparator {
    type State = MenuSeparatorState;

    fn build_into_menu(self, parent: &MenuParent) -> Self::State {
        // gio renders a divider between adjacent sections. We
        // implement <menu_separator/> by closing the parent's
        // currently-open section: subsequent items pick up a fresh
        // section, and the gap between the two becomes a visible
        // divider.
        match parent {
            MenuParent::Menu { cursor, .. } => {
                cursor.flush();
            }
            MenuParent::Bar(_) => panic!(
                "<menu_separator/> must be a child of <menu>, not \
                 directly under <menu_bar>."
            ),
        }
        // Nothing to retain — the divider is encoded in the
        // section topology, not in any standalone widget.
        MenuSeparatorState {}
    }
}
