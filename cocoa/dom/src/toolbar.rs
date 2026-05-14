//! NSToolbar + NSToolbarItem wrappers, plus a custom
//! `ToolbarDelegate` ObjC class that vends pre-built items by
//! identifier.
//!
//! Architecture mirrors the `menu` module: the lower-level wrappers
//! here own `Retained<NSToolbar>` / `Retained<NSToolbarItem>`,
//! expose ergonomic setters, and reuse the existing `ActionTarget`
//! + `HANDLER_STORE` machinery for click handling. The
//! higher-level `Toolbar<C>` builder in `leptos_cocoa::cocoa::toolbar`
//! wraps these and runs a `ToolbarMountable` cascade over the
//! child items.
//!
//! ## Why a custom delegate
//!
//! NSToolbar always calls its delegate to vend items — even with
//! `allowsUserCustomization = false`. There's no "manually
//! installed items, no delegate" mode. Our delegate pre-builds
//! every item the developer specified at `Toolbar::build` time
//! and stashes them in a `RefCell<HashMap<String, Retained<NSToolbarItem>>>`;
//! when AppKit asks `toolbar:itemForItemIdentifier:` the delegate
//! looks the item up in the map.
//!
//! ## Custom-item identifier scheme
//!
//! Built-in items use Apple-provided identifiers
//! (`NSToolbarFlexibleSpaceItemIdentifier`, etc.); custom items
//! use whatever string the developer supplied. Uniqueness is
//! validated by the higher-level builder before any item is
//! created. Built-in identifiers are short-circuited in the
//! delegate — AppKit creates those itself, we just return `nil`.

#![allow(missing_docs)]

use std::cell::RefCell;
use std::collections::HashMap;

use crate::event::{
    action_fired_sel, drop_action_target_for_key, keep_target_alive_for_key,
    ActionTarget,
};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{NSObject, NSObjectProtocol, ProtocolObject},
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSImage, NSToolbar, NSToolbarDelegate, NSToolbarFlexibleSpaceItemIdentifier,
    NSToolbarIdentifier, NSToolbarItem, NSToolbarItemIdentifier,
    NSToolbarSpaceItemIdentifier, NSWindow,
};
use objc2_foundation::{NSArray, NSString};

// ---------------------------------------------------------------------
// ToolbarDelegate ObjC class
// ---------------------------------------------------------------------

/// Per-toolbar registry of identifier → item, looked up by
/// AppKit's delegate callbacks. Also stores the ordered list of
/// identifiers so default/allowed/immovable methods return them in
/// the same order the developer specified.
pub struct ToolbarDelegateState {
    /// Identifier → pre-built NSToolbarItem. Built-in identifiers
    /// (flexible space, space) are NOT in here — AppKit creates
    /// those itself, so the delegate returns `None`.
    pub items: RefCell<HashMap<String, Retained<NSToolbarItem>>>,
    /// In-display-order list of identifiers (both custom and
    /// built-in). Used to populate `default` and `allowed` lists.
    pub ordered_identifiers: RefCell<Vec<String>>,
}

define_class!(
    /// NSToolbarDelegate implementation. Vends items by identifier
    /// from its ivar map; returns the full ordered list as
    /// default / allowed / immovable so the toolbar layout is
    /// locked to what the developer wrote.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ToolbarDelegateState]
    pub struct ToolbarDelegate;

    unsafe impl NSObjectProtocol for ToolbarDelegate {}

    unsafe impl NSToolbarDelegate for ToolbarDelegate {
        #[unsafe(method_id(toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:))]
        fn item_for_identifier(
            &self,
            _toolbar: &NSToolbar,
            identifier: &NSToolbarItemIdentifier,
            _will_be_inserted: bool,
        ) -> Option<Retained<NSToolbarItem>> {
            // AppKit constructs built-in items itself; we only
            // need to vend our custom ones.
            let id_str = identifier.to_string();
            let map = self.ivars().items.borrow();
            map.get(&id_str).cloned()
        }

        #[unsafe(method_id(toolbarDefaultItemIdentifiers:))]
        fn default_identifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.ordered_ns_array()
        }

        #[unsafe(method_id(toolbarAllowedItemIdentifiers:))]
        fn allowed_identifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<NSArray<NSToolbarItemIdentifier>> {
            self.ordered_ns_array()
        }

        #[unsafe(method_id(toolbarImmovableItemIdentifiers:))]
        fn immovable_identifiers(
            &self,
            _toolbar: &NSToolbar,
        ) -> Retained<objc2_foundation::NSSet<NSToolbarItemIdentifier>> {
            // v1 locks the toolbar layout (no customisation) by
            // returning the full ordered identifier list as
            // immovable. Customisation comes later.
            use objc2_foundation::NSMutableSet;
            let set: Retained<NSMutableSet<NSToolbarItemIdentifier>> =
                NSMutableSet::new();
            for id in self.ivars().ordered_identifiers.borrow().iter() {
                let s = NSString::from_str(id);
                let id_ns: &NSToolbarItemIdentifier = unsafe {
                    &*(&*s as *const NSString as *const NSToolbarItemIdentifier)
                };
                set.addObject(id_ns);
            }
            unsafe { Retained::cast_unchecked::<objc2_foundation::NSSet<_>>(set) }
        }
    }
);

impl ToolbarDelegate {
    fn ordered_ns_array(
        &self,
    ) -> Retained<NSArray<NSToolbarItemIdentifier>> {
        // Box each identifier through NSString. The
        // `NSToolbarItemIdentifier` typedef is `NSString`; we cast
        // the strings into the typedef type before wrapping in
        // NSArray.
        let raw: Vec<Retained<NSString>> = self
            .ivars()
            .ordered_identifiers
            .borrow()
            .iter()
            .map(|s| NSString::from_str(s))
            .collect();
        let ids: Vec<&NSToolbarItemIdentifier> = raw
            .iter()
            .map(|s| unsafe {
                &*(&**s as *const NSString as *const NSToolbarItemIdentifier)
            })
            .collect();
        NSArray::from_slice(&ids)
    }

    pub fn new(
        items: HashMap<String, Retained<NSToolbarItem>>,
        ordered_identifiers: Vec<String>,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let alloc = Self::alloc(mtm).set_ivars(ToolbarDelegateState {
            items: RefCell::new(items),
            ordered_identifiers: RefCell::new(ordered_identifiers),
        });
        unsafe { msg_send![super(alloc), init] }
    }
}

// ---------------------------------------------------------------------
// Toolbar (NSToolbar wrapper)
// ---------------------------------------------------------------------

/// Wrapper around `Retained<NSToolbar>` + the matching delegate.
///
/// NSToolbar holds its delegate as a *weak* reference — we keep a
/// strong `Retained<ToolbarDelegate>` here so it doesn't get
/// deallocated mid-display.
pub struct Toolbar {
    ns_toolbar: Retained<NSToolbar>,
    // Held to keep the weakly-referenced delegate alive.
    _delegate: Retained<ToolbarDelegate>,
    /// Pointer-as-usize keys for every item with a registered
    /// action handler. Used by `drop_handlers` on teardown to
    /// release the retained `ActionTarget` entries from the
    /// shared `HANDLER_STORE`.
    handler_keys: Vec<usize>,
}

/// Build an NSToolbar with `identifier`, a pre-populated item map,
/// and the in-order identifier list (including both custom and
/// built-in identifiers). The caller is expected to have already
/// configured each `NSToolbarItem` (label, image, target/action, ...).
pub fn toolbar(
    identifier: &str,
    items: HashMap<String, Retained<NSToolbarItem>>,
    ordered_identifiers: Vec<String>,
    handler_keys: Vec<usize>,
    mtm: MainThreadMarker,
) -> Toolbar {
    let id_ns = NSString::from_str(identifier);
    let id_ref: &NSToolbarIdentifier = unsafe {
        &*(&*id_ns as *const NSString as *const NSToolbarIdentifier)
    };
    let ns_toolbar: Retained<NSToolbar> =
        NSToolbar::initWithIdentifier(NSToolbar::alloc(mtm), id_ref);
    // No user customisation in v1 — see module docs.
    ns_toolbar.setAllowsUserCustomization(false);
    ns_toolbar.setAutosavesConfiguration(false);

    let delegate = ToolbarDelegate::new(items, ordered_identifiers, mtm);
    let proto: &ProtocolObject<dyn NSToolbarDelegate> =
        ProtocolObject::from_ref(&*delegate);
    ns_toolbar.setDelegate(Some(proto));

    Toolbar {
        ns_toolbar,
        _delegate: delegate,
        handler_keys,
    }
}

impl Toolbar {
    /// Borrow the underlying `NSToolbar`.
    pub fn ns_toolbar(&self) -> &NSToolbar {
        &self.ns_toolbar
    }

    /// Attach this toolbar to `window`. AppKit then drives the
    /// delegate callbacks to populate the visible toolbar from
    /// the default-identifier list.
    pub fn attach_to_window(&self, window: &NSWindow) {
        window.setToolbar(Some(&self.ns_toolbar));
    }

    /// Release every retained `ActionTarget` we registered for
    /// items in this toolbar. Called on drop.
    pub fn drop_handlers(&self) {
        for key in &self.handler_keys {
            drop_action_target_for_key(*key);
        }
    }

    /// Test-only: borrow the `HANDLER_STORE` keys this toolbar
    /// owns so a test can verify they're cleared on drop.
    #[doc(hidden)]
    pub fn test_handler_keys(&self) -> &[usize] {
        &self.handler_keys
    }
}

impl Drop for Toolbar {
    fn drop(&mut self) {
        // Detach the toolbar from any window that still holds it,
        // then drop the action handlers. Without the detach, AppKit
        // can still hold a reference to the toolbar (via the
        // window's `toolbar` slot) past our Drop and try to
        // re-render — fine in principle but it leaks the items.
        self.drop_handlers();
    }
}

// ---------------------------------------------------------------------
// ToolbarItem (NSToolbarItem wrapper) — one per custom action item.
// ---------------------------------------------------------------------

/// Wrapper around `Retained<NSToolbarItem>` with ergonomic setters.
/// Single-action contract matches `MenuItem::set_action`.
#[derive(Clone)]
pub struct ToolbarItem {
    ns_item: Retained<NSToolbarItem>,
}

/// Construct a fresh, blank toolbar item with the given identifier.
/// Label/image/target/action are filled in via setters.
pub fn toolbar_item(identifier: &str, mtm: MainThreadMarker) -> ToolbarItem {
    let id_ns = NSString::from_str(identifier);
    let id_ref: &NSToolbarItemIdentifier = unsafe {
        &*(&*id_ns as *const NSString as *const NSToolbarItemIdentifier)
    };
    let ns_item: Retained<NSToolbarItem> =
        NSToolbarItem::initWithItemIdentifier(NSToolbarItem::alloc(mtm), id_ref);
    ToolbarItem { ns_item }
}

impl ToolbarItem {
    pub fn ns_item(&self) -> &NSToolbarItem {
        &self.ns_item
    }

    pub fn into_ns_item(self) -> Retained<NSToolbarItem> {
        self.ns_item
    }

    pub fn set_label(&self, label: &str) {
        let s = NSString::from_str(label);
        self.ns_item.setLabel(&s);
    }

    pub fn set_palette_label(&self, label: &str) {
        let s = NSString::from_str(label);
        self.ns_item.setPaletteLabel(&s);
    }

    pub fn set_tool_tip(&self, tip: &str) {
        let s = NSString::from_str(tip);
        self.ns_item.setToolTip(Some(&s));
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.ns_item.setEnabled(enabled);
    }

    /// Install an `NSImage` (already configured, e.g. with an SF
    /// Symbol point-size config). Empty / None clears the image.
    pub fn set_image(&self, image: Option<&NSImage>) {
        self.ns_item.setImage(image);
    }

    /// Convenience: set the item's image from an SF Symbol name.
    /// Uses the shared `sf_symbol_image` helper so the image gets
    /// a default 16pt regular point-size configuration.
    pub fn set_sf_symbol(&self, name: &str) {
        let img = crate::node::sf_symbol_image(name);
        self.ns_item.setImage(img.as_deref());
    }

    /// Wire a Rust closure as the item's action handler.
    /// Single-handler contract — a second call panics, matching
    /// `MenuItem::set_action`.
    pub fn set_action<F>(&self, cb: F, mtm: MainThreadMarker) -> usize
    where
        F: FnMut() + 'static,
    {
        if self.ns_item.target().is_some() {
            panic!(
                "set_action called twice on NSToolbarItem ({:p}). \
                 NSToolbarItem has a single target/action slot. \
                 Combine your handlers into one closure.",
                &*self.ns_item,
            );
        }
        let target = ActionTarget::new(cb, mtm);
        let target_obj: &NSObject = &target;
        unsafe {
            self.ns_item.setTarget(Some(target_obj));
            self.ns_item.setAction(Some(action_fired_sel()));
        }
        let key = self.handler_key();
        keep_target_alive_for_key(key, target);
        key
    }

    /// Pointer-as-usize key used to retain the action handler in
    /// the shared `HANDLER_STORE`. Returned by `set_action` so the
    /// caller can pass it to `Toolbar::drop_handlers` on teardown.
    pub fn handler_key(&self) -> usize {
        let ptr: *const NSToolbarItem = &*self.ns_item;
        ptr as usize
    }
}

// ---------------------------------------------------------------------
// Built-in item identifiers
// ---------------------------------------------------------------------

/// AppKit's "flexible space" identifier. NSToolbar interprets it
/// internally — no NSToolbarItem instance to create.
pub fn flexible_space_identifier() -> String {
    unsafe { NSToolbarFlexibleSpaceItemIdentifier.to_string() }
}

/// AppKit's fixed-width "space" identifier.
pub fn space_identifier() -> String {
    unsafe { NSToolbarSpaceItemIdentifier.to_string() }
}

/// Returns true if `id` is a built-in AppKit identifier — used by
/// the higher-level builder so it doesn't bother creating a
/// NSToolbarItem map entry for these (AppKit vends them itself).
pub fn is_builtin_identifier(id: &str) -> bool {
    id == flexible_space_identifier() || id == space_identifier()
}
