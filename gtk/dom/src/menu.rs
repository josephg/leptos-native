//! Imperative GTK menu wrappers — the GTK4 sibling of
//! `cocoa_dom::menu`.
//!
//! Architecture: GTK's menu model is declarative — a tree of
//! `gio::MenuItem`s grouped under `gio::Menu` (the mutable
//! implementation of the `gio::MenuModel` abstract base class), with
//! item activation routed via named `gio::Action`s on the
//! `gtk::Application`'s action group. The application-bar UI is
//! installed via `gtk_application_set_menubar(menu)`; the desktop
//! shell renders it (a traditional menubar on Cinnamon/MATE, the
//! "globally shared" menubar slot under GNOME on macOS-like
//! configurations, etc.).
//!
//! Each `<menu_item>` we build allocates a fresh action name
//! (`app.menuitem_<N>` where `N` is a process-local counter),
//! registers a `gio::SimpleAction` whose `connect_activate` fires
//! the user closure, and stashes the action name on the
//! `gio::MenuItem` via `set_detailed_action`. Accelerator strings
//! (`<Primary>r`) are bound via
//! `Application::set_accels_for_action`.
//!
//! `gio::MenuItem` is *immutable once added* — to change its
//! displayed title reactively, we have to `remove(index)` /
//! `insert_item(index, new)` it. That bookkeeping lives in the
//! higher `leptos_gtk::gtk::menu` layer.

use gtk4::prelude::*;
use renderer::menu::Modifiers;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------
// MenuBar
// ---------------------------------------------------------------------

/// Application-wide menu bar. Wraps a `gio::Menu` that becomes the
/// root model passed to `gtk_application_set_menubar`. Cloneable
/// (both fields are ref-counted glib objects) so the Render layer
/// can stash a copy in the reactive-title closure that re-inserts
/// the submenu wrapper on signal changes.
#[derive(Clone)]
pub struct MenuBar {
    menu: gio::Menu,
    app:  gtk4::Application,
}

impl MenuBar {
    /// Borrow the underlying `gio::Menu`.
    pub fn gio_menu(&self) -> &gio::Menu {
        &self.menu
    }

    /// Borrow the parent `gtk::Application`.
    pub fn app(&self) -> &gtk4::Application {
        &self.app
    }

    /// Install this menu as the application's menu bar.
    pub fn install(&self) {
        self.app.set_menubar(Some(&self.menu));
    }

    /// Append a submenu under a label. Returns the index at which
    /// the submenu was inserted so the caller can later
    /// remove/replace it (e.g. when reactive titles re-emit the
    /// wrapper item).
    pub fn append_submenu(&self, label: &str, sub: &Menu) -> usize {
        let idx = self.menu.n_items() as usize;
        self.menu.append_submenu(Some(label), &sub.menu);
        idx
    }

    /// Replace the wrapper item at `idx` (label + submenu pair).
    /// Used when a submenu's reactive `title` changes — the
    /// underlying `gio::MenuItem` is immutable so we evict and
    /// re-insert.
    pub fn replace_submenu(&self, idx: usize, label: &str, sub: &Menu) {
        self.menu.remove(idx as i32);
        let item = gio::MenuItem::new_submenu(Some(label), &sub.menu);
        self.menu.insert_item(idx as i32, &item);
    }
}

/// Construct a fresh, empty `MenuBar`. Doesn't install — call
/// [`MenuBar::install`] for that.
pub fn menu_bar(app: &gtk4::Application) -> MenuBar {
    MenuBar {
        menu: gio::Menu::new(),
        app:  app.clone(),
    }
}

// ---------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------

/// A single submenu — `gio::Menu` exposed via its parent's
/// `append_submenu(label, &menu)`. Cloneable (the underlying
/// `gio::Menu` is a ref-counted glib object).
#[derive(Clone)]
pub struct Menu {
    menu: gio::Menu,
}

/// Create a fresh, empty submenu.
pub fn menu() -> Menu {
    Menu { menu: gio::Menu::new() }
}

impl Menu {
    /// Borrow the underlying `gio::Menu`.
    pub fn gio_menu(&self) -> &gio::Menu {
        &self.menu
    }

    /// Append an action item (leaf). Returns the inserted index.
    pub fn append_item(&self, item: &MenuItem) -> usize {
        let idx = self.menu.n_items() as usize;
        self.menu.append_item(&item.item);
        idx
    }

    /// Append a nested submenu. Returns the inserted index of the
    /// wrapper item.
    pub fn append_submenu(&self, label: &str, sub: &Menu) -> usize {
        let idx = self.menu.n_items() as usize;
        self.menu.append_submenu(Some(label), &sub.menu);
        idx
    }

    /// Append a section separator. A "section" in `gio::Menu` is
    /// the closest analog to AppKit's separator — adjacent items in
    /// the same section render without a divider, items in
    /// different sections get separated. Cheaper than fiddling
    /// with `<Span>` items.
    pub fn append_section(&self, sub: &Menu) -> usize {
        let idx = self.menu.n_items() as usize;
        self.menu.append_section(None, &sub.menu);
        idx
    }

    /// Replace a leaf item at `idx`. Used for reactive title
    /// updates (since `gio::MenuItem` is immutable).
    pub fn replace_item(&self, idx: usize, item: &MenuItem) {
        self.menu.remove(idx as i32);
        self.menu.insert_item(idx as i32, &item.item);
    }

    /// Replace a wrapper-submenu item at `idx`.
    pub fn replace_submenu(&self, idx: usize, label: &str, sub: &Menu) {
        self.menu.remove(idx as i32);
        let item = gio::MenuItem::new_submenu(Some(label), &sub.menu);
        self.menu.insert_item(idx as i32, &item);
    }
}

// ---------------------------------------------------------------------
// MenuItem
// ---------------------------------------------------------------------

/// Leaf menu command. Carries its `gio::MenuItem`, a private
/// `gio::SimpleAction` (so the closure can fire), and the action
/// name (for accel binding + reactive title rebuilds).
///
/// Cloneable — clones share the underlying `gio::MenuItem` /
/// `gio::SimpleAction` (refcounted) and the `Rc<Cell<bool>>`
/// single-handler guard, so the `set_action`-twice panic catches
/// double-installs through any clone.
#[derive(Clone)]
pub struct MenuItem {
    item:           gio::MenuItem,
    action:         gio::SimpleAction,
    action_name:    String,
    /// Single-handler guard — flipped to `true` on first
    /// `set_action` call; subsequent calls panic to match the
    /// cocoa rule. Shared across clones via `Rc`.
    action_wired:   std::rc::Rc<std::cell::Cell<bool>>,
}

/// Process-wide counter for action-name generation. Names look like
/// `menuitem_42` and are scoped under `app.` when referenced from
/// menu items / accels.
static NEXT_ACTION_ID: AtomicUsize = AtomicUsize::new(0);

/// Create a fresh menu item with a unique `app.menuitem_N` action.
/// The action is **not** activated by anything yet — call
/// [`MenuItem::set_action`] to wire a closure.
///
/// The action is also not registered on any `gtk::Application` —
/// the caller (the leptos_gtk Render impl) does that after
/// configuring the action.
pub fn menu_item() -> MenuItem {
    new_menu_item(/* checkable */ false)
}

/// Create a fresh menu item whose underlying `gio::SimpleAction`
/// carries a boolean state, so the menu renderer shows a check-
/// mark column for it. Callers wanting [`MenuItem::set_checked`] to
/// actually do anything must use this variant — the plain
/// [`menu_item`] action has no state and the check-column flag is
/// silently ignored.
///
/// State starts as `false`. Activating the item via the desktop
/// shell auto-toggles the state per `gio::SimpleAction`'s default
/// activate handler; layer the user closure on top via
/// [`MenuItem::set_action`] and call [`MenuItem::set_checked`]
/// from a reactive effect to re-assert state on signal changes.
pub fn menu_item_checkable() -> MenuItem {
    new_menu_item(/* checkable */ true)
}

fn new_menu_item(checkable: bool) -> MenuItem {
    let id = NEXT_ACTION_ID.fetch_add(1, Ordering::Relaxed);
    let action_name = format!("menuitem_{}", id);
    let action = if checkable {
        let a = gio::SimpleAction::new_stateful(
            &action_name,
            None,
            &false.to_variant(),
        );
        // Suppress the default change-state handler. Without this,
        // activating a boolean-stateful action auto-flips its state
        // — which would visually toggle the checkmark on click,
        // independent of whether the user's `on:action` closure
        // actually mutated the bound signal. A `checked=move || sig.get()`
        // binding would then drift out of sync with `sig`. Matching
        // cocoa's behaviour: the checkmark strictly mirrors the
        // reactive setter; click only fires the user closure.
        a.connect_change_state(|_action, _value| {
            // intentional no-op — see comment above
        });
        a
    } else {
        gio::SimpleAction::new(&action_name, None)
    };

    let item = gio::MenuItem::new(Some(""), None);
    item.set_detailed_action(&format!("app.{}", action_name));

    MenuItem {
        item,
        action,
        action_name,
        action_wired: std::rc::Rc::new(std::cell::Cell::new(false)),
    }
}

impl MenuItem {
    /// Borrow the underlying `gio::MenuItem`.
    pub fn gio_item(&self) -> &gio::MenuItem {
        &self.item
    }

    /// Borrow the underlying `gio::SimpleAction`. Caller is
    /// responsible for `app.add_action(...)`ing it.
    pub fn action(&self) -> &gio::SimpleAction {
        &self.action
    }

    /// The `app.menuitem_N` action name (without the `app.` prefix).
    pub fn action_name(&self) -> &str {
        &self.action_name
    }

    /// Update the displayed title. `gio::MenuItem` is immutable
    /// once added to a `gio::Menu`, so reactive title changes must
    /// instead replace the item at its index — see
    /// [`Menu::replace_item`].
    pub fn set_label(&self, label: &str) {
        self.item.set_label(Some(label));
    }

    /// Enable / disable the item via the action's `enabled` flag.
    pub fn set_enabled(&self, b: bool) {
        self.action.set_enabled(b);
    }

    /// Show or hide the check-mark indicator. Only effective when
    /// the item was constructed via [`menu_item_checkable`] (which
    /// gives the underlying action a boolean state). Calling this
    /// on a non-checkable item is silently ignored — the action
    /// has no state to set.
    ///
    /// `set_state` doesn't fire `change_state` handlers, so reactive
    /// re-assertion (driven by `checked=move || sig.get()`) doesn't
    /// trigger any spurious closures.
    pub fn set_checked(&self, b: bool) {
        if self.action.state_type().is_some() {
            self.action.set_state(&b.to_variant());
        }
    }

    /// Set the keyboard accelerator. `key` is e.g. `"r"`, `"F1"`,
    /// `"plus"` — the GDK-accepted key name (also passes through
    /// `gtk::accelerator_parse`). Calls
    /// `app.set_accels_for_action`, so this needs an `app` handle
    /// — passed in by the caller (the leptos_gtk Render impl which
    /// has it in scope).
    pub fn set_shortcut(
        &self,
        app: &gtk4::Application,
        key: &str,
        mods: Modifiers,
    ) {
        let accel = format!("{}{}", modifiers_to_accel(mods), key);
        let accels = [accel.as_str()];
        app.set_accels_for_action(&format!("app.{}", self.action_name), &accels);
    }

    /// Wire a Rust closure as the item's activation handler.
    /// Single-handler contract: a second call panics rather than
    /// fanning out (matches the cocoa rule). The check is via the
    /// shared `action_wired` flag, so calling `set_action` through
    /// a clone of the same item also panics.
    pub fn set_action<F>(&self, app: &gtk4::Application, cb: F)
    where
        F: FnMut() + 'static,
    {
        if self.action_wired.get() {
            panic!(
                "set_action called twice on the same MenuItem (action \
                 \"{}\"). gio::SimpleAction::connect_activate would stack \
                 the handlers — combine into one closure instead.",
                self.action_name,
            );
        }
        self.action_wired.set(true);

        let cb = std::cell::RefCell::new(cb);
        self.action.connect_activate(move |_, _| {
            if let Ok(mut cb) = cb.try_borrow_mut() {
                cb();
            } else {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[gtk_dom::menu] reentrant action handler skipped"
                );
            }
        });
        // Register the action on the application. add_action with the
        // same name replaces; we shouldn't hit that case because of
        // the action_wired guard, but the underlying behavior is
        // defined.
        app.add_action(&self.action);
    }
}

// ---------------------------------------------------------------------
// Modifiers translation
// ---------------------------------------------------------------------

/// Build the GTK accel-string prefix for a Modifiers bag.
///
/// `command` translates to `<Primary>` (the portable Cmd/Ctrl
/// alias). Note `<Control>` and `<Primary>` are distinct on macOS
/// but identical on Linux — apps that want explicit Ctrl on macOS
/// should set `control` rather than `command`.
fn modifiers_to_accel(m: Modifiers) -> String {
    let mut s = String::new();
    if m.command { s.push_str("<Primary>"); }
    if m.shift   { s.push_str("<Shift>");   }
    if m.option  { s.push_str("<Alt>");     }
    if m.control { s.push_str("<Control>"); }
    s
}
