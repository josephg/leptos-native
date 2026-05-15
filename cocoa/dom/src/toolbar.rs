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
    NSImage, NSSearchField, NSSearchToolbarItem, NSToolbar, NSToolbarDelegate,
    NSToolbarFlexibleSpaceItemIdentifier, NSToolbarIdentifier, NSToolbarItem,
    NSToolbarItemIdentifier, NSToolbarPrintItemIdentifier,
    NSToolbarSidebarTrackingSeparatorItemIdentifier,
    NSToolbarSpaceItemIdentifier, NSToolbarToggleSidebarItemIdentifier, NSView,
    NSWindow,
};
use objc2_foundation::{NSArray, NSString};

use crate::Element;

// ---------------------------------------------------------------------
// ToolbarDelegate ObjC class
// ---------------------------------------------------------------------

/// Bookkeeping kept per custom toolbar item: the NSToolbarItem
/// itself plus any HANDLER_STORE key for its action target. The
/// `Drop` impl releases the action target retain when the
/// registration is dropped — either at toolbar teardown (the
/// whole ivar HashMap drops) or when a single item is removed
/// dynamically via [`Toolbar::remove_item`].
pub struct ToolbarItemRegistration {
    pub ns_item: Retained<NSToolbarItem>,
    /// HANDLER_STORE key for the action target. `None` for items
    /// with no `on:action` handler.
    pub handler_key: Option<usize>,
    /// For `NSSearchToolbarItem`-backed items: an [`Element`]
    /// wrapping the embedded `NSSearchField`. Held so the
    /// text-field handler-store entries
    /// (`on_text_change` etc.) get released on drop via
    /// [`crate::event::drop_handlers_for`]. `None` for regular
    /// [`NSToolbarItem`] entries.
    pub search_element: Option<Element>,
}

impl Drop for ToolbarItemRegistration {
    fn drop(&mut self) {
        if let Some(key) = self.handler_key {
            drop_action_target_for_key(key);
        }
        if let Some(el) = self.search_element.take() {
            // The element isn't in any Taffy tree (NSSearchField is
            // owned by the NSSearchToolbarItem, not mounted as a
            // child of anything), so just clean up the handler
            // store. The NSView itself is released via the
            // NSToolbarItem above.
            crate::event::drop_handlers_for(el.ns_view());
        }
    }
}

/// Per-toolbar registry of identifier → item registration, looked
/// up by AppKit's delegate callbacks. Also stores the ordered list
/// of identifiers so default/allowed/immovable methods return them
/// in the same order the developer specified.
///
/// Both fields wrap their state in `RefCell` so dynamic
/// `insert_item` / `remove_item` calls (via `ToolbarHandle`) can
/// mutate after build. Delegate callbacks only borrow immutably.
pub struct ToolbarDelegateState {
    /// Identifier → registration. Built-in identifiers
    /// (flexible space, space) are NOT in here — AppKit creates
    /// those itself, so the delegate returns `None` for them.
    pub items: RefCell<HashMap<String, ToolbarItemRegistration>>,
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
            self.ivars()
                .items
                .borrow()
                .get(&id_str)
                .map(|reg| reg.ns_item.clone())
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
        items: HashMap<String, ToolbarItemRegistration>,
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
/// deallocated mid-display. Also stores the [`MainThreadMarker`]
/// needed for any future main-thread-only calls (e.g. building
/// items on the fly via [`Self::insert_item`]).
///
/// ## Drop order
///
/// Field destruction follows Rust's declared field order:
///   1. `ns_toolbar` drops — AppKit releases its internal item
///      retains.
///   2. `delegate` drops — its ivar `items` HashMap drops, calling
///      `Drop` on each `ToolbarItemRegistration`. Each Drop
///      releases the HANDLER_STORE retain on its action target,
///      which lets NSToolbarItem's separate target retain be the
///      last one — the item then drops and releases its target,
///      and the target finally deallocates.
///
/// Do not reorder the fields.
pub struct Toolbar {
    ns_toolbar: Retained<NSToolbar>,
    /// Held to keep the weakly-referenced delegate alive — and to
    /// own the per-item registrations via its ivar HashMap.
    delegate: Retained<ToolbarDelegate>,
    mtm: MainThreadMarker,
}

/// Build an NSToolbar with `identifier`, a pre-populated item map,
/// and the in-order identifier list (including both custom and
/// built-in identifiers). The caller is expected to have already
/// configured each `NSToolbarItem` (label, image, target/action, ...).
pub fn toolbar(
    identifier: &str,
    items: HashMap<String, ToolbarItemRegistration>,
    ordered_identifiers: Vec<String>,
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
        delegate,
        mtm,
    }
}

impl Toolbar {
    /// Borrow the underlying `NSToolbar`.
    pub fn ns_toolbar(&self) -> &NSToolbar {
        &self.ns_toolbar
    }

    /// Clone the retained `NSToolbar` pointer. Used by the
    /// higher-level builder to capture the toolbar inside
    /// reactive setter closures.
    pub fn ns_toolbar_retained(&self) -> Retained<NSToolbar> {
        self.ns_toolbar.clone()
    }

    /// Clone the retained delegate pointer. Used by
    /// `ToolbarHandle` to mutate the delegate's items map after
    /// the toolbar has been built.
    pub fn delegate_retained(&self) -> Retained<ToolbarDelegate> {
        self.delegate.clone()
    }

    /// The MainThreadMarker used to build this toolbar — passed
    /// through to `ToolbarHandle` so it can build new
    /// NSToolbarItems on demand.
    pub fn mtm(&self) -> MainThreadMarker {
        self.mtm
    }

    /// Attach this toolbar to `window`. AppKit then drives the
    /// delegate callbacks to populate the visible toolbar from
    /// the default-identifier list.
    pub fn attach_to_window(&self, window: &NSWindow) {
        window.setToolbar(Some(&self.ns_toolbar));
    }

    /// Controls how items are presented: icon + label (default),
    /// icon only, or label only.
    pub fn set_display_mode(&self, mode: ToolbarDisplayMode) {
        self.ns_toolbar.setDisplayMode(mode.to_appkit());
    }

    /// Insert a pre-built `ToolbarItemRegistration` at `index`
    /// (saturating to the current count). Updates the delegate's
    /// item map + ordered list, then calls
    /// `NSToolbar.insertItemWithItemIdentifier:atIndex:` so AppKit
    /// re-queries the delegate and renders the new item.
    ///
    /// Returns `Err` with the registration unchanged if `identifier`
    /// is already present (duplicate identifiers are a hard
    /// invariant — NSToolbar uses them as map keys).
    pub fn insert_item(
        &self,
        identifier: String,
        registration: ToolbarItemRegistration,
        index: usize,
    ) -> Result<(), ToolbarItemRegistration> {
        {
            let items = self.delegate.ivars().items.borrow();
            if items.contains_key(&identifier) {
                return Err(registration);
            }
        }
        let count = self.delegate.ivars().ordered_identifiers.borrow().len();
        let idx = index.min(count);

        self.delegate
            .ivars()
            .items
            .borrow_mut()
            .insert(identifier.clone(), registration);
        self.delegate
            .ivars()
            .ordered_identifiers
            .borrow_mut()
            .insert(idx, identifier.clone());

        // Tell AppKit to install the item. It calls back through
        // the delegate to fetch the NSToolbarItem from our map.
        let id_ns = NSString::from_str(&identifier);
        let id_ref: &NSToolbarItemIdentifier = unsafe {
            &*(&*id_ns as *const NSString as *const NSToolbarItemIdentifier)
        };
        self.ns_toolbar
            .insertItemWithItemIdentifier_atIndex(id_ref, idx as isize);
        Ok(())
    }

    /// Remove a previously-inserted item by identifier. Returns
    /// `true` if the item was found and removed; `false` if no
    /// such identifier exists.
    ///
    /// Drops the registration (which releases the action target
    /// retain via `Drop`) and calls
    /// `NSToolbar.removeItemAtIndex:` so AppKit removes the item
    /// from the visible toolbar.
    pub fn remove_item(&self, identifier: &str) -> bool {
        // Find the visible index from the ordered list.
        let idx = {
            let ordered = self.delegate.ivars().ordered_identifiers.borrow();
            match ordered.iter().position(|s| s == identifier) {
                Some(i) => i,
                None => return false,
            }
        };
        // Drop the registration first — its Drop releases the
        // HANDLER_STORE retain. NSToolbar still holds its own
        // retain on the NSToolbarItem until we call
        // removeItemAtIndex.
        let _registration = self
            .delegate
            .ivars()
            .items
            .borrow_mut()
            .remove(identifier);
        self.delegate
            .ivars()
            .ordered_identifiers
            .borrow_mut()
            .remove(idx);
        self.ns_toolbar.removeItemAtIndex(idx as isize);
        true
    }

    /// Does the toolbar currently contain an item with this
    /// identifier? Useful for `<Show>`-style reactive add/remove
    /// patterns to avoid duplicate insertions.
    pub fn contains_item(&self, identifier: &str) -> bool {
        self.delegate.ivars().items.borrow().contains_key(identifier)
    }

    /// Test-only: read back the HANDLER_STORE key of the n-th
    /// registered item with an action handler (in insertion order).
    /// Used by `drop_releases_action_target` to verify cleanup.
    #[doc(hidden)]
    pub fn test_handler_key_at(&self, n: usize) -> Option<usize> {
        let ordered = self.delegate.ivars().ordered_identifiers.borrow();
        let items = self.delegate.ivars().items.borrow();
        ordered
            .iter()
            .filter_map(|id| items.get(id).and_then(|r| r.handler_key))
            .nth(n)
    }
}

// No explicit `Drop` impl — the delegate's ivar HashMap drops
// every `ToolbarItemRegistration` automatically when the toolbar
// is destroyed, and each registration's own Drop releases its
// HANDLER_STORE retain. See the struct-level doc-comment for the
// full drop-order chain.

// ---------------------------------------------------------------------
// ToolbarItem (NSToolbarItem wrapper) — one per custom action item.
// ---------------------------------------------------------------------

/// Wrapper around `Retained<NSToolbarItem>` with ergonomic setters.
/// Single-action contract matches `MenuItem::set_action`.
///
/// `Clone` is shallow: clones share the same NSToolbarItem and the
/// same `last_icon` diff cell (via `Rc`), so the reactive install
/// closures (which clone the wrapper to capture into each effect)
/// all observe the same "last-applied" icon.
#[derive(Clone)]
pub struct ToolbarItem {
    ns_item: Retained<NSToolbarItem>,
    /// Last [`crate::Icon`] applied via [`Self::set_icon`] (or
    /// its named-primitive shorthands [`Self::set_sf_symbol`] /
    /// [`Self::set_image_path`], which delegate). Used as the
    /// single source of truth for diffing — re-applying the same
    /// `Icon` still recreates the underlying `NSImage` and calls
    /// `setImage:`, which triggers an NSToolbar re-layout that
    /// can flicker adjacent items.
    ///
    /// `None` means no icon currently set (either never set, or
    /// most recently set via [`Self::set_image`] which bypasses
    /// the Icon-shaped identity).
    last_icon: std::rc::Rc<std::cell::RefCell<Option<crate::Icon>>>,
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
    ToolbarItem {
        ns_item,
        last_icon: std::rc::Rc::new(std::cell::RefCell::new(None)),
    }
}

impl ToolbarItem {
    pub fn ns_item(&self) -> &NSToolbarItem {
        &self.ns_item
    }

    pub fn into_ns_item(self) -> Retained<NSToolbarItem> {
        self.ns_item
    }

    pub fn set_label(&self, label: &str) {
        // Diff before mutating — re-setting the same label
        // triggers an NSToolbar re-layout pass that visibly
        // flickers adjacent items.
        if self.ns_item.label().to_string() == label {
            return;
        }
        self.ns_item.setLabel(&NSString::from_str(label));
    }

    pub fn set_palette_label(&self, label: &str) {
        if self.ns_item.paletteLabel().to_string() == label {
            return;
        }
        self.ns_item.setPaletteLabel(&NSString::from_str(label));
    }

    pub fn set_tool_tip(&self, tip: &str) {
        let current: String = self
            .ns_item
            .toolTip()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if current == tip {
            return;
        }
        self.ns_item.setToolTip(Some(&NSString::from_str(tip)));
    }

    pub fn set_enabled(&self, enabled: bool) {
        // Opt out of NSToolbar's auto-validation. By default
        // `NSToolbarItem.autovalidates = true` and the toolbar
        // periodically calls `validate` on each item; the default
        // validation logic re-enables any item whose target
        // responds to its action (which ours always does, via
        // `ActionTarget`). Without disabling autovalidation, our
        // explicit `setEnabled(false)` is immediately undone on
        // the next validation pass, making the reactive `enabled`
        // attribute appear to do nothing. Setting this once is
        // idempotent — calling set_enabled implies the caller is
        // managing the state themselves.
        self.ns_item.setAutovalidates(false);

        // Diff before mutating — `setEnabled:` with the same
        // value still triggers AppKit to re-evaluate the item's
        // appearance (re-apply the dim filter on the image), which
        // visibly flashes the icon. The reactive `enabled=` effect
        // re-runs whenever its tracked signal changes, so the
        // closure can return the same boolean multiple times in a
        // row (e.g. pressing "Reset" when the counter is already
        // zero).
        if self.ns_item.isEnabled() == enabled {
            return;
        }
        self.ns_item.setEnabled(enabled);
    }

    /// Install an `NSImage` directly, bypassing the [`crate::Icon`]
    /// abstraction. Empty / `None` clears the image. Resets the
    /// Icon-diff state to `None` — subsequent
    /// [`Self::set_icon`] calls will re-apply unconditionally,
    /// since we don't track arbitrary `NSImage` identities.
    pub fn set_image(&self, image: Option<&NSImage>) {
        self.ns_item.setImage(image);
        *self.last_icon.borrow_mut() = None;
    }

    /// Set the item's icon from the unified [`crate::Icon`] enum.
    /// Single source of truth for both SF Symbol and file-path
    /// images.
    ///
    /// Diffs against the last `Icon` applied — re-emitting the
    /// same variant + payload is a no-op, avoiding the NSToolbar
    /// re-layout that `setImage:` would otherwise trigger.
    /// Switching variants (SF Symbol → file path, or vice versa)
    /// replaces the image atomically; no stale "both kinds set at
    /// once" state is possible.
    ///
    /// `None` clears the image entirely.
    pub fn set_icon(&self, icon: Option<&crate::Icon>) {
        // Top-level diff — one cell, covers every transition.
        if self.last_icon.borrow().as_ref() == icon {
            return;
        }
        use objc2::AllocAnyThread;
        use objc2_app_kit::NSImage;
        match icon {
            Some(crate::Icon::SfSymbol(name)) => {
                let img = crate::node::sf_symbol_image(name);
                self.ns_item.setImage(img.as_deref());
            }
            Some(crate::Icon::Image(path)) => {
                if path.is_empty() {
                    self.ns_item.setImage(None);
                } else {
                    let path_ns = NSString::from_str(path);
                    let image = NSImage::initWithContentsOfFile(
                        NSImage::alloc(),
                        &path_ns,
                    );
                    self.ns_item.setImage(image.as_deref());
                }
            }
            None => {
                self.ns_item.setImage(None);
            }
        }
        *self.last_icon.borrow_mut() = icon.cloned();
    }

    /// Convenience: set the item's image from an SF Symbol name.
    /// Equivalent to `set_icon(Some(&Icon::sf_symbol(name)))`.
    pub fn set_sf_symbol(&self, name: &str) {
        self.set_icon(Some(&crate::Icon::sf_symbol(name)));
    }

    /// Load an image from a filesystem path. Empty path / failed
    /// load clears the slot. Equivalent to
    /// `set_icon(Some(&Icon::image(path)))`.
    pub fn set_image_path(&self, path: &str) {
        self.set_icon(Some(&crate::Icon::image(path)));
    }

    /// Toggle the item's bordered button appearance. With
    /// `bordered=true` AppKit draws the modern button-style
    /// background on hover/press; with `bordered=false` the item
    /// is a flat icon. macOS 11+.
    pub fn set_bordered(&self, bordered: bool) {
        self.ns_item.setBordered(bordered);
    }

    /// Install an arbitrary `NSView` as the item's content (replaces
    /// the default icon + label rendering with whatever view you
    /// supply). NSToolbar will set the view's frame to fit the
    /// item's slot — typically the view should self-lay-out via
    /// autoresizing or be a single self-contained control
    /// (`NSTextField`, `NSSegmentedControl`, ...).
    ///
    /// Pairs with `setMinSize:` / `setMaxSize:` for controls that
    /// need a specific size; we don't set those automatically since
    /// AppKit infers sensible defaults from the view's intrinsic
    /// content size.
    pub fn set_view(&self, view: Option<&NSView>) {
        self.ns_item.setView(view);
    }

    /// Mark the item as a navigation control (back/forward).
    /// Navigational items get distinct positioning and styling in
    /// modern macOS toolbars. macOS 12+.
    pub fn set_navigational(&self, navigational: bool) {
        self.ns_item.setNavigational(navigational);
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

/// Toggle an `NSToolbar`'s visibility, no-op'ing on redundant
/// calls. Free function rather than a method so the
/// install-effect closure in the high-level builder can call it
/// with just a `Retained<NSToolbar>` clone (the dom `Toolbar`
/// struct isn't `Clone`-able).
pub fn set_toolbar_visible(ns_toolbar: &NSToolbar, visible: bool) {
    if ns_toolbar.isVisible() == visible {
        return;
    }
    ns_toolbar.setVisible(visible);
}

// ---------------------------------------------------------------------
// SearchToolbarItem (NSSearchToolbarItem wrapper)
// ---------------------------------------------------------------------

/// Wrapper around `Retained<NSSearchToolbarItem>` + an [`Element`]
/// view of its embedded `NSSearchField`.
///
/// `NSSearchToolbarItem` (macOS 11+) is the native, idiomatic way to
/// host a search field in a toolbar. It supplies the
/// magnifying-glass icon, the clear (×) button, recent-searches
/// support, and toolbar-specific expand/collapse behavior — all
/// without us writing chrome.
///
/// The search field that AppKit creates and owns inside the item is
/// wrapped as a [`crate::Element`] so the existing text-field
/// setters (`set_string_attribute(Placeholder|Value, _)`,
/// `on_text_change`, ...) work on it directly — `NSSearchField`
/// IS-A `NSTextField`, so the existing `downcast::<NSTextField>`
/// paths catch it automatically.
pub struct SearchToolbarItem {
    /// Upcast `Retained<NSToolbarItem>` so the registration can
    /// store it alongside regular items. Dynamic dispatch preserves
    /// the NSSearchToolbarItem subclass identity.
    ns_item: Retained<NSToolbarItem>,
    /// Element wrapping the embedded `NSSearchField`. The element's
    /// Node has no Taffy handle (the field is laid out by AppKit
    /// inside the toolbar item, not by Taffy).
    search_element: Element,
    /// Auto Layout constraint pinning the search field's width.
    /// Lazily created by [`Self::set_search_field_width`]; once
    /// installed, subsequent calls just update its `constant`.
    width_constraint: std::cell::RefCell<
        Option<objc2::rc::Retained<objc2_app_kit::NSLayoutConstraint>>,
    >,
}

/// Construct a fresh `NSSearchToolbarItem`. The embedded
/// `NSSearchField` is the one AppKit creates for us; we don't
/// override it. Label / tool tip / placeholder etc. are wired via
/// the returned wrapper.
pub fn search_toolbar_item(
    identifier: &str,
    mtm: MainThreadMarker,
) -> SearchToolbarItem {
    use crate::node::{Node, NodeKind};
    use taffy::Style;

    let id_ns = NSString::from_str(identifier);
    let id_ref: &NSToolbarItemIdentifier = unsafe {
        &*(&*id_ns as *const NSString as *const NSToolbarItemIdentifier)
    };
    let ns_search_item: Retained<NSSearchToolbarItem> =
        NSSearchToolbarItem::initWithItemIdentifier(
            NSSearchToolbarItem::alloc(mtm),
            id_ref,
        );

    // The embedded NSSearchField — AppKit owns it; we just borrow.
    let search_field: Retained<NSSearchField> = ns_search_item.searchField();

    // Fire the search field's `action` only when the user
    // *commits* (Return key / clear button), not on every
    // keystroke. NSSearchField's default is to send the action
    // continuously (debounced), which makes `on:action` indistinguishable
    // from `on:input`. Setting `sendsWholeSearchString = true`
    // restores the expected "commit semantics" — `on:action`
    // becomes a real commit hook, `on:input` is still per-keystroke.
    search_field.setSendsWholeSearchString(true);

    // Wrap the search field as an Element so existing string-attr /
    // event-handler plumbing applies. No Taffy handle — NSToolbar
    // controls the layout.
    let node = Node::from_view(search_field, NodeKind::Element, Style::default());
    let search_element = Element::from_node_unchecked(node);

    // Upcast to NSToolbarItem for storage in
    // `ToolbarItemRegistration.ns_item`. ObjC dynamic dispatch
    // keeps the NSSearchToolbarItem identity through the upcast.
    let ns_item: Retained<NSToolbarItem> =
        unsafe { Retained::cast_unchecked(ns_search_item) };

    SearchToolbarItem {
        ns_item,
        search_element,
        width_constraint: std::cell::RefCell::new(None),
    }
}

impl SearchToolbarItem {
    pub fn ns_item(&self) -> &NSToolbarItem {
        &self.ns_item
    }

    pub fn ns_item_retained(&self) -> Retained<NSToolbarItem> {
        self.ns_item.clone()
    }

    pub fn search_element(&self) -> &Element {
        &self.search_element
    }

    /// Consume the wrapper into `(NSToolbarItem, Element)` for the
    /// caller to stash separately.
    pub fn into_parts(self) -> (Retained<NSToolbarItem>, Element) {
        (self.ns_item, self.search_element)
    }

    pub fn set_label(&self, label: &str) {
        // Diff before mutating — re-setting the same label
        // triggers an NSToolbar re-layout pass that visibly
        // flickers adjacent items.
        if self.ns_item.label().to_string() == label {
            return;
        }
        self.ns_item.setLabel(&NSString::from_str(label));
    }

    pub fn set_palette_label(&self, label: &str) {
        if self.ns_item.paletteLabel().to_string() == label {
            return;
        }
        self.ns_item.setPaletteLabel(&NSString::from_str(label));
    }

    pub fn set_tool_tip(&self, tip: &str) {
        let current: String = self
            .ns_item
            .toolTip()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if current == tip {
            return;
        }
        self.ns_item.setToolTip(Some(&NSString::from_str(tip)));
    }

    pub fn set_enabled(&self, enabled: bool) {
        // Opt out of NSToolbar's auto-validation. By default
        // `NSToolbarItem.autovalidates = true` and the toolbar
        // periodically calls `validate` on each item; the default
        // validation logic re-enables any item whose target
        // responds to its action (which ours always does, via
        // `ActionTarget`). Without disabling autovalidation, our
        // explicit `setEnabled(false)` is immediately undone on
        // the next validation pass, making the reactive `enabled`
        // attribute appear to do nothing. Setting this once is
        // idempotent — calling set_enabled implies the caller is
        // managing the state themselves.
        self.ns_item.setAutovalidates(false);

        // Diff before mutating — `setEnabled:` with the same
        // value still triggers AppKit to re-evaluate the item's
        // appearance (re-apply the dim filter on the image), which
        // visibly flashes the icon. The reactive `enabled=` effect
        // re-runs whenever its tracked signal changes, so the
        // closure can return the same boolean multiple times in a
        // row (e.g. pressing "Reset" when the counter is already
        // zero).
        if self.ns_item.isEnabled() == enabled {
            return;
        }
        self.ns_item.setEnabled(enabled);
    }

    /// Set the search field's preferred width when the toolbar
    /// expands it. NSSearchToolbarItem defaults to a fairly narrow
    /// field (~140pt); set this to give the field more room. The
    /// toolbar still collapses to an icon-only state when the
    /// window is too narrow.
    pub fn set_preferred_width_for_search_field(&self, width: f64) {
        // Downcast back to NSSearchToolbarItem to reach the
        // subclass-specific setter. The upcast in
        // `search_toolbar_item` was unchecked, so the runtime
        // class is still NSSearchToolbarItem.
        let any: &objc2::runtime::AnyObject = self.ns_item.as_ref();
        if let Some(s) = any.downcast_ref::<NSSearchToolbarItem>() {
            s.setPreferredWidthForSearchField(width);
        }
    }

    /// Pin the embedded `NSSearchField`'s width via Auto Layout.
    ///
    /// `NSSearchToolbarItem.preferredWidthForSearchField` only
    /// applies **when the field has keyboard focus**. Unfocused,
    /// the field shrinks back to its compact natural width — so
    /// any click that moves focus elsewhere (e.g. clicking the
    /// sidebar toggle button) makes the search field visibly
    /// shrink, and the next click into it expands it again.
    ///
    /// Setting a `widthAnchor.constraintEqualToConstant` directly
    /// on the search field locks its width across both states.
    /// First call creates and activates the constraint; subsequent
    /// calls update its `constant` in place (much cheaper than
    /// deactivating + adding a fresh constraint).
    pub fn set_search_field_width(&self, width: f64) {
        let any: &objc2::runtime::AnyObject = self.ns_item.as_ref();
        let Some(s) = any.downcast_ref::<NSSearchToolbarItem>() else {
            return;
        };
        let field = s.searchField();
        let mut slot = self.width_constraint.borrow_mut();
        if let Some(existing) = slot.as_ref() {
            existing.setConstant(width);
        } else {
            let constraint = field
                .widthAnchor()
                .constraintEqualToConstant(width);
            constraint.setActive(true);
            *slot = Some(constraint);
        }
    }
}

// ---------------------------------------------------------------------
// Built-in item identifiers
// ---------------------------------------------------------------------

/// AppKit's "flexible space" identifier. NSToolbar interprets it
/// internally — no NSToolbarItem instance to create.
///
/// Allocates each call (NSString → String). Cheap enough at build
/// time (toolbars are built once); callers that need to hot-loop
/// against the value should cache.
pub fn flexible_space_identifier() -> String {
    unsafe { NSToolbarFlexibleSpaceItemIdentifier.to_string() }
}

/// AppKit's fixed-width "space" identifier.
pub fn space_identifier() -> String {
    unsafe { NSToolbarSpaceItemIdentifier.to_string() }
}

/// AppKit's "toggle sidebar" identifier. AppKit creates an
/// NSToolbarItem that targets the responder chain with
/// `toggleSidebar:` — `NSSplitViewController` handles it
/// automatically, so the user just needs to put the toolbar
/// inside a split-window mount.
pub fn toggle_sidebar_identifier() -> String {
    unsafe { NSToolbarToggleSidebarItemIdentifier.to_string() }
}

/// AppKit's "sidebar tracking separator" identifier (macOS 11+).
/// A separator that auto-aligns its horizontal position with the
/// first divider of the window's `NSSplitView`. Pairs naturally
/// with split-window mounts.
pub fn sidebar_tracking_separator_identifier() -> String {
    unsafe { NSToolbarSidebarTrackingSeparatorItemIdentifier.to_string() }
}

/// AppKit's "Print…" identifier. Sends `printDocument:` up the
/// responder chain.
pub fn print_identifier() -> String {
    unsafe { NSToolbarPrintItemIdentifier.to_string() }
}

// ---------------------------------------------------------------------
// Display mode (port-local renderer-agnostic enum)
// ---------------------------------------------------------------------

/// How NSToolbar lays out each item's icon and label. Maps 1:1 to
/// AppKit's `NSToolbarDisplayMode`, exposed here as a renderer-
/// agnostic enum so callers don't have to pull `objc2_app_kit` in.
///
/// On modern macOS the AppKit-default mode (`Default`) is
/// functionally identical to `IconAndLabel`; both produce
/// icon-above-label items. The `Default` variant exists so
/// callers can express "let AppKit decide" without us second-
/// guessing future system-level changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarDisplayMode {
    /// System default — typically icon and label.
    Default,
    /// Icon plus label below.
    IconAndLabel,
    /// Icon only — labels collapse to tooltips on hover.
    IconOnly,
    /// Label only — no icons.
    LabelOnly,
}

impl Default for ToolbarDisplayMode {
    fn default() -> Self {
        Self::Default
    }
}

impl ToolbarDisplayMode {
    /// Convert to AppKit's `NSToolbarDisplayMode`. Public so the
    /// `leptos_cocoa` builder can dispatch reactive setters
    /// against `NSToolbar.setDisplayMode:` directly.
    pub fn to_appkit(self) -> objc2_app_kit::NSToolbarDisplayMode {
        use objc2_app_kit::NSToolbarDisplayMode as M;
        match self {
            Self::Default => M::Default,
            Self::IconAndLabel => M::IconAndLabel,
            Self::IconOnly => M::IconOnly,
            Self::LabelOnly => M::LabelOnly,
        }
    }
}

// ---------------------------------------------------------------------
// Window toolbar style (lives on NSWindow, not NSToolbar)
// ---------------------------------------------------------------------

/// Modern macOS toolbar appearance, set on the *window* via
/// `setToolbarStyle:` (macOS 11+). Different styles place the
/// toolbar differently relative to the title bar and tweak its
/// vertical sizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowToolbarStyle {
    /// System default — AppKit picks one based on window content.
    Automatic,
    /// Toolbar in its own band below the title bar — the classic
    /// legacy look.
    Expanded,
    /// Compact toolbar styled for Preferences-style windows.
    Preference,
    /// Modern integrated look — toolbar blends with the title bar.
    Unified,
    /// Narrower variant of `Unified` for compact windows.
    UnifiedCompact,
}

impl WindowToolbarStyle {
    /// Convert to AppKit's `NSWindowToolbarStyle`.
    pub fn to_appkit(self) -> objc2_app_kit::NSWindowToolbarStyle {
        use objc2_app_kit::NSWindowToolbarStyle as S;
        match self {
            Self::Automatic => S::Automatic,
            Self::Expanded => S::Expanded,
            Self::Preference => S::Preference,
            Self::Unified => S::Unified,
            Self::UnifiedCompact => S::UnifiedCompact,
        }
    }
}
