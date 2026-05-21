//! Tests for the unified `Icon` enum and the `set_icon` setters
//! it drives on `ToolbarItem` / `MenuItem`.
//!
//! Covers:
//! - Each variant populates / clears `NSToolbarItem.image` /
//!   `NSMenuItem.image` correctly.
//! - Transitions between variants atomically replace the image
//!   (the bug the unified-Icon refactor was introduced to
//!   prevent).
//! - The top-level diff bails on a redundant re-application —
//!   the underlying `NSImage` pointer is preserved.
//! - `set_icon(None)` clears the image and the diff state.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::{
    menu::{self as dom_menu},
    toolbar::{self as dom_toolbar},
    Icon,
};
use objc2::rc::Retained;
use objc2_app_kit::{NSImage, NSMenuItem, NSToolbarItem};

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn ns_image_ptr(image: &Option<Retained<NSImage>>) -> Option<*const NSImage> {
    image.as_ref().map(|r| &**r as *const NSImage)
}

fn tb_image_ptr(item: &NSToolbarItem) -> Option<*const NSImage> {
    ns_image_ptr(&item.image())
}

fn menu_image_ptr(item: &NSMenuItem) -> Option<*const NSImage> {
    ns_image_ptr(&item.image())
}

// ---------------------------------------------------------------------
// ToolbarItem
// ---------------------------------------------------------------------

fn toolbar_set_icon_sf_symbol_populates() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);
    item.set_icon(Some(&Icon::sf_symbol("plus")));
    let ns = item.ns_item();
    assert!(
        ns.image().is_some(),
        "Icon::SfSymbol should set NSToolbarItem.image"
    );
}

fn toolbar_set_icon_empty_image_path_clears() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);
    item.set_icon(Some(&Icon::sf_symbol("plus")));
    assert!(item.ns_item().image().is_some());

    item.set_icon(Some(&Icon::image("")));
    assert!(
        item.ns_item().image().is_none(),
        "Icon::image(empty) should clear NSToolbarItem.image"
    );
}

fn toolbar_set_icon_none_clears() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);
    item.set_icon(Some(&Icon::sf_symbol("plus")));
    assert!(item.ns_item().image().is_some());

    item.set_icon(None);
    assert!(
        item.ns_item().image().is_none(),
        "set_icon(None) should clear NSToolbarItem.image"
    );
}

/// Walk through SF Symbol → Image(empty) → SF Symbol again. The
/// image should re-populate on the final transition, NOT be
/// suppressed by a stale diff state from the first SF Symbol.
fn toolbar_set_icon_variant_transitions_atomic() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);

    item.set_icon(Some(&Icon::sf_symbol("plus")));
    let after_sf = tb_image_ptr(item.ns_item());
    assert!(after_sf.is_some(), "step 1: SF symbol image should populate");

    item.set_icon(Some(&Icon::image("")));
    assert!(
        item.ns_item().image().is_none(),
        "step 2: empty image path should clear"
    );

    item.set_icon(Some(&Icon::sf_symbol("minus")));
    let after_sf2 = tb_image_ptr(item.ns_item());
    assert!(
        after_sf2.is_some(),
        "step 3: returning to SF symbol should re-populate"
    );
    assert_ne!(
        after_sf, after_sf2,
        "step 3: a different SF symbol should produce a different NSImage \
         (the first symbol's diff state must not suppress the new one)"
    );
}

/// Re-applying the same Icon is a no-op — the NSImage pointer
/// must not change, proving the top-level `last_icon` diff fires.
fn toolbar_set_icon_redundant_is_noop() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);

    item.set_icon(Some(&Icon::sf_symbol("plus")));
    let before = tb_image_ptr(item.ns_item());
    assert!(before.is_some());

    item.set_icon(Some(&Icon::sf_symbol("plus")));
    let after = tb_image_ptr(item.ns_item());

    assert_eq!(
        before, after,
        "redundant set_icon with the same Icon must be a no-op; \
         a new NSImage pointer indicates the diff failed to fire"
    );
}

/// `set_sf_symbol(name)` / `set_image_path(path)` are shorthands
/// for `set_icon(Some(&Icon::*(…)))`. They should hit the same
/// diff path.
fn toolbar_named_shorthands_share_diff() {
    let mtm = common::test_mtm();
    let item = dom_toolbar::toolbar_item("test", mtm);

    item.set_sf_symbol("plus");
    let before = tb_image_ptr(item.ns_item());
    item.set_sf_symbol("plus");
    let after = tb_image_ptr(item.ns_item());
    assert_eq!(
        before, after,
        "set_sf_symbol(same) must hit the same diff as set_icon"
    );
}

// ---------------------------------------------------------------------
// MenuItem
// ---------------------------------------------------------------------

fn menu_set_icon_sf_symbol_populates() {
    let mtm = common::test_mtm();
    let item = dom_menu::menu_item(mtm);
    item.set_icon(Some(&Icon::sf_symbol("doc.badge.plus")));
    assert!(
        item.ns_item().image().is_some(),
        "Icon::SfSymbol should set NSMenuItem.image"
    );
}

fn menu_set_icon_none_clears() {
    let mtm = common::test_mtm();
    let item = dom_menu::menu_item(mtm);
    item.set_icon(Some(&Icon::sf_symbol("doc.badge.plus")));
    assert!(item.ns_item().image().is_some());

    item.set_icon(None);
    assert!(
        item.ns_item().image().is_none(),
        "set_icon(None) should clear NSMenuItem.image"
    );
}

fn menu_set_icon_variant_transitions_atomic() {
    let mtm = common::test_mtm();
    let item = dom_menu::menu_item(mtm);

    item.set_icon(Some(&Icon::sf_symbol("plus.circle")));
    let after_sf = menu_image_ptr(item.ns_item());
    assert!(after_sf.is_some());

    item.set_icon(Some(&Icon::image("")));
    assert!(item.ns_item().image().is_none());

    item.set_icon(Some(&Icon::sf_symbol("minus.circle")));
    let after_sf2 = menu_image_ptr(item.ns_item());
    assert!(after_sf2.is_some());
    assert_ne!(
        after_sf, after_sf2,
        "different SF symbols should produce different NSImages \
         on the menu item too"
    );
}

fn menu_set_icon_redundant_is_noop() {
    let mtm = common::test_mtm();
    let item = dom_menu::menu_item(mtm);

    item.set_icon(Some(&Icon::sf_symbol("plus.circle")));
    let before = menu_image_ptr(item.ns_item());
    item.set_icon(Some(&Icon::sf_symbol("plus.circle")));
    let after = menu_image_ptr(item.ns_item());

    assert_eq!(
        before, after,
        "redundant menu set_icon must be a no-op"
    );
}

fn main() {
    common::run_tests(&[
        (
            "toolbar_set_icon_sf_symbol_populates",
            toolbar_set_icon_sf_symbol_populates,
        ),
        (
            "toolbar_set_icon_empty_image_path_clears",
            toolbar_set_icon_empty_image_path_clears,
        ),
        ("toolbar_set_icon_none_clears", toolbar_set_icon_none_clears),
        (
            "toolbar_set_icon_variant_transitions_atomic",
            toolbar_set_icon_variant_transitions_atomic,
        ),
        (
            "toolbar_set_icon_redundant_is_noop",
            toolbar_set_icon_redundant_is_noop,
        ),
        (
            "toolbar_named_shorthands_share_diff",
            toolbar_named_shorthands_share_diff,
        ),
        (
            "menu_set_icon_sf_symbol_populates",
            menu_set_icon_sf_symbol_populates,
        ),
        ("menu_set_icon_none_clears", menu_set_icon_none_clears),
        (
            "menu_set_icon_variant_transitions_atomic",
            menu_set_icon_variant_transitions_atomic,
        ),
        (
            "menu_set_icon_redundant_is_noop",
            menu_set_icon_redundant_is_noop,
        ),
    ]);
}
