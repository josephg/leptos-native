//! Imperative wrappers around AppKit's `NSMenu` / `NSMenuItem`.
//!
//! Lowest layer of the menu story — analogous to [`crate::node`] for
//! NSView, but for the menu side. The higher
//! `leptos_cocoa::cocoa::menu` layer turns these into `Render`-shaped
//! builders that the `view!{}` macro can drive.
//!
//! Threading: every method here must run on the main thread, same
//! as the rest of `cocoa_dom`. The opaque `MenuBar` / `Menu` /
//! `MenuItem` handles wrap `Retained<NSMenu>` / `Retained<NSMenuItem>`
//! directly; the AppKit objects are themselves not `Send`. Callers
//! drive everything from `MainThreadOnly` contexts (the runtime
//! `MainThreadMarker` is passed in to construction functions).

use crate::event::{
    action_fired_sel, drop_action_target_for_key, keep_target_alive_for_key,
    ActionTarget,
};
use objc2::{rc::Retained, runtime::NSObject, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn,
    NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::NSString;
use renderer::menu::Modifiers;

// ---------------------------------------------------------------------
// MenuBar
// ---------------------------------------------------------------------

/// An AppKit main-menu (`NSApp.mainMenu`) wrapped to look like a
/// list of submenus. Constructed via [`menu_bar`]; install onto the
/// running NSApplication via [`MenuBar::install`].
///
/// Internally just an `NSMenu` — AppKit doesn't have a separate
/// "menu bar" type, the main menu is just whatever `NSMenu` is set
/// as `mainMenu`. The header-bar bookkeeping (one NSMenuItem per
/// submenu, whose `submenu:` is the actual `NSMenu`) is done in
/// [`MenuBar::append_menu`].
pub struct MenuBar {
    ns_menu: Retained<NSMenu>,
}

impl MenuBar {
    /// Wrap an existing `NSMenu` for tests / introspection. Most
    /// callers should use [`menu_bar`].
    pub fn from_ns_menu(ns_menu: Retained<NSMenu>) -> Self {
        Self { ns_menu }
    }

    /// Borrow the underlying `NSMenu`.
    pub fn ns_menu(&self) -> &NSMenu {
        &self.ns_menu
    }

    /// Install this menu as `NSApp.mainMenu`. Overwrites any
    /// previously installed menu — by design, the v1 plan replaces
    /// the baseline `App + Edit` menu (installed during `init_app`)
    /// rather than appending to it.
    pub fn install(&self, app: &NSApplication) {
        app.setMainMenu(Some(&self.ns_menu));
    }

    /// Append a submenu. AppKit's submenu-in-menubar shape is:
    /// one `NSMenuItem` per top-level entry, whose `submenu:` is
    /// the actual `NSMenu` of items. We construct that wrapper
    /// item here, set its title to the submenu's title (so the
    /// menu bar shows it), and stash the item itself so callers
    /// can later remove it.
    pub fn append_menu(&self, m: &Menu, mtm: MainThreadMarker) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&m.ns_menu.title());
        item.setSubmenu(Some(&m.ns_menu));
        self.ns_menu.addItem(&item);
        item
    }

    /// Remove a previously-installed submenu by its header item.
    pub fn remove_menu_item(&self, item: &NSMenuItem) {
        self.ns_menu.removeItem(item);
    }
}

/// Construct a fresh, empty `MenuBar`. The returned bar is **not**
/// installed onto NSApp yet — call [`MenuBar::install`] for that.
pub fn menu_bar(mtm: MainThreadMarker) -> MenuBar {
    MenuBar { ns_menu: NSMenu::new(mtm) }
}

// ---------------------------------------------------------------------
// Menu
// ---------------------------------------------------------------------

/// A single submenu (the second level of the menu hierarchy on
/// macOS — `<menu>` in the user-facing API). Wraps an `NSMenu`.
/// A `Menu` can itself be appended as a *sub*-submenu to another
/// `Menu` via [`Menu::append_submenu`].
#[derive(Clone)]
pub struct Menu {
    ns_menu: Retained<NSMenu>,
}

/// Construct an empty submenu with the given title. The title is
/// what appears in the menu bar when this menu is appended via
/// [`MenuBar::append_menu`], or as the label of the wrapper item
/// when nested via [`Menu::append_submenu`].
pub fn menu(title: &str, mtm: MainThreadMarker) -> Menu {
    let ns_menu = NSMenu::new(mtm);
    ns_menu.setTitle(&NSString::from_str(title));
    Menu { ns_menu }
}

impl Menu {
    /// Borrow the underlying `NSMenu`.
    pub fn ns_menu(&self) -> &NSMenu {
        &self.ns_menu
    }

    /// Update the menu's title. This is the title AppKit shows
    /// when the menu is reached via the keyboard API; for menu-bar
    /// / parent-menu display the *wrapper* `NSMenuItem`'s title
    /// (returned by [`MenuBar::append_menu`] / [`Menu::append_submenu`])
    /// is what gets read — AppKit copies the submenu's title to the
    /// wrapper at attach time, but does **not** keep them in sync
    /// after that. Callers that want a dynamic label must update
    /// both.
    pub fn set_title(&self, t: &str) {
        self.ns_menu.setTitle(&NSString::from_str(t));
    }

    /// Append an item to the menu. The item must have been
    /// constructed via [`menu_item`] / [`menu_separator`] — those
    /// are the only shapes we support today.
    pub fn append_item(&self, item: &MenuItem) {
        self.ns_menu.addItem(&item.ns_item);
    }

    /// Append a nested submenu. Creates the wrapper `NSMenuItem`
    /// (title = `sub`'s current title, `submenu:` = `sub`) and
    /// adds it. Returns the wrapper item so the caller can later
    /// detach it.
    pub fn append_submenu(
        &self,
        sub: &Menu,
        mtm: MainThreadMarker,
    ) -> Retained<NSMenuItem> {
        let item = NSMenuItem::new(mtm);
        item.setTitle(&sub.ns_menu.title());
        item.setSubmenu(Some(&sub.ns_menu));
        self.ns_menu.addItem(&item);
        item
    }

    /// Remove an item (leaf or submenu wrapper) previously
    /// installed under this menu.
    pub fn remove_item(&self, item: &NSMenuItem) {
        self.ns_menu.removeItem(item);
    }
}

// ---------------------------------------------------------------------
// MenuItem
// ---------------------------------------------------------------------

/// A leaf menu command. Wraps `NSMenuItem`. Construct via
/// [`menu_item`]; configure via the chainable setters; fire user
/// closures via [`MenuItem::set_action`].
///
/// Action wiring re-uses [`crate::event::ActionTarget`] — the same
/// target/action object that backs `NSButton on:click`. A second
/// `set_action` call on the same item panics rather than silently
/// overwriting, matching `on_control_action`'s contract.
#[derive(Clone)]
pub struct MenuItem {
    ns_item: Retained<NSMenuItem>,
}

/// Construct a fresh, blank `MenuItem`. Title is empty, no key
/// equivalent, no action. The higher-level builder fills these in
/// from its `MaybeReactive` slots.
pub fn menu_item(mtm: MainThreadMarker) -> MenuItem {
    MenuItem { ns_item: NSMenuItem::new(mtm) }
}

/// Construct a separator menu item — a thin horizontal line used
/// to divide groups of related commands. AppKit's
/// `+[NSMenuItem separatorItem]` returns a singleton item kind,
/// distinct from regular menu items.
pub fn menu_separator(mtm: MainThreadMarker) -> MenuItem {
    MenuItem { ns_item: NSMenuItem::separatorItem(mtm) }
}

impl MenuItem {
    /// Borrow the underlying `NSMenuItem`.
    pub fn ns_item(&self) -> &NSMenuItem {
        &self.ns_item
    }

    /// Set the item's displayed title.
    pub fn set_title(&self, t: &str) {
        self.ns_item.setTitle(&NSString::from_str(t));
    }

    /// Toggle whether the item is enabled (i.e. user can click /
    /// trigger via keyboard shortcut). Disabled items render
    /// greyed out and don't fire their action.
    pub fn set_enabled(&self, b: bool) {
        self.ns_item.setEnabled(b);
    }

    /// Set the item's check-mark state (the "On/Off" column on
    /// the left edge of the menu). Used by toggle-style items
    /// like "Show Sidebar".
    pub fn set_checked(&self, b: bool) {
        self.ns_item.setState(if b {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }

    /// Bind a keyboard shortcut to the item. `key` is the
    /// unmodified character (e.g. `"s"` for ⌘S); `mods` is the
    /// modifier bag.
    ///
    /// Empty `key` clears any existing shortcut.
    pub fn set_shortcut(&self, key: &str, mods: Modifiers) {
        self.ns_item
            .setKeyEquivalent(&NSString::from_str(key));
        self.ns_item
            .setKeyEquivalentModifierMask(modifiers_to_ns(mods));
    }

    /// Wire a Rust closure as the item's action handler.
    ///
    /// Re-uses [`crate::event::ActionTarget`] (the same ObjC class
    /// NSButton's target/action wiring uses). The retain is kept
    /// alive in the per-control thread-local handler store keyed
    /// by the NSMenuItem pointer; see
    /// [`crate::event::keep_target_alive`] / [`drop_handlers`].
    ///
    /// Single-handler contract: a second `set_action` call on the
    /// same item panics rather than silently overwriting. Combine
    /// your handlers into one closure if you need fan-out.
    pub fn set_action<F>(&self, cb: F, mtm: MainThreadMarker)
    where
        F: FnMut() + 'static,
    {
        if let Some(existing) = self.ns_item.target() {
            panic!(
                "set_action called twice on the same NSMenuItem ({:p}). \
                 NSMenuItem has a single target/action slot. Workaround: \
                 combine your handlers into one closure. Existing \
                 target: {:p}",
                &*self.ns_item, &*existing,
            );
        }
        let target = ActionTarget::new(cb, mtm);
        let target_obj: &NSObject = &target;
        unsafe {
            self.ns_item.setTarget(Some(target_obj));
            self.ns_item.setAction(Some(action_fired_sel()));
        }
        // Share the HANDLER_STORE keyspace with NSControl-backed
        // handlers, keyed on the NSMenuItem's pointer-as-usize.
        // The store is purely a retain-keeper; the key never gets
        // dereferenced, so it doesn't matter that NSMenuItem isn't
        // an NSView. (Earlier we fabricated a `&NSView` from the
        // NSMenuItem pointer — that was UB.)
        keep_target_alive_for_key(self.handler_key(), target);
    }

    /// Drop any retained action handler attached to this item.
    /// Mirrors [`crate::event::drop_handlers_for`] but only touches
    /// the action-target store — NSMenuItems don't participate in
    /// `TEXT_FIELD_STORE` / `TEXT_VIEW_STORE`.
    pub fn drop_handlers(&self) {
        drop_action_target_for_key(self.handler_key());
    }

    /// Pointer-as-`usize` key used for the HANDLER_STORE entry.
    /// Pulled into a single method so both `set_action` and
    /// `drop_handlers` see the same value, and no other call site
    /// can pick a different key.
    fn handler_key(&self) -> usize {
        let ptr: *const NSMenuItem = &*self.ns_item;
        ptr as usize
    }
}

// ---------------------------------------------------------------------
// Modifiers translation
// ---------------------------------------------------------------------

/// Translate a portable [`Modifiers`] into AppKit's bitflag enum.
fn modifiers_to_ns(m: Modifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    if m.command {
        flags |= NSEventModifierFlags::Command;
    }
    if m.shift {
        flags |= NSEventModifierFlags::Shift;
    }
    if m.option {
        flags |= NSEventModifierFlags::Option;
    }
    if m.control {
        flags |= NSEventModifierFlags::Control;
    }
    flags
}
