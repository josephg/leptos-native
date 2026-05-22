//! Tests for the imperative `cocoa_dom::menu` layer (the
//! `MenuBar` / `Menu` / `MenuItem` wrappers around NSMenu /
//! NSMenuItem and the `Modifiers` translation).
//!
//! These don't run the menu against AppKit's display — they
//! observe the constructed objects directly to verify the wiring
//! is what we expect (titles, key equivalents, modifier flags,
//! state, action-target installation).

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::menu::{menu, menu_bar, menu_item, menu_separator};
use objc2_app_kit::{NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags};
use leptos_native::renderer::menu::Modifiers;

// ---------------------------------------------------------------------
// MenuBar / Menu construction
// ---------------------------------------------------------------------

fn menu_bar_starts_empty() {
    let mtm = common::test_mtm();
    let bar = menu_bar(mtm);
    assert_eq!(bar.ns_menu().itemArray().len(), 0);
}

fn menu_construction_sets_title() {
    let mtm = common::test_mtm();
    let m = menu("File", mtm);
    assert_eq!(m.ns_menu().title().to_string(), "File");
}

fn menu_set_title_updates_ns_menu_title() {
    let mtm = common::test_mtm();
    let m = menu("Old", mtm);
    m.set_title("New");
    assert_eq!(m.ns_menu().title().to_string(), "New");
}

fn append_menu_creates_wrapper_with_submenu_pointer() {
    let mtm = common::test_mtm();
    let bar = menu_bar(mtm);
    let file = menu("File", mtm);
    let wrapper = bar.append_menu(&file, mtm);

    // The wrapper item now lives in the bar.
    let items = bar.ns_menu().itemArray();
    assert_eq!(items.len(), 1);

    // The wrapper's title was copied from the submenu's title at
    // attach time (this is what shows in the menu bar).
    assert_eq!(wrapper.title().to_string(), "File");

    // The wrapper's submenu points at the underlying NSMenu.
    let sub = wrapper.submenu().expect("wrapper should have submenu");
    let sub_ptr: *const _ = &*sub;
    let file_ptr: *const _ = file.ns_menu();
    assert_eq!(sub_ptr, file_ptr);
}

fn append_submenu_in_menu_creates_nested_wrapper() {
    let mtm = common::test_mtm();
    let parent = menu("File", mtm);
    let recent = menu("Open Recent", mtm);
    let wrapper = parent.append_submenu(&recent, mtm);

    assert_eq!(parent.ns_menu().itemArray().len(), 1);
    assert_eq!(wrapper.title().to_string(), "Open Recent");
    assert!(wrapper.submenu().is_some());
}

// ---------------------------------------------------------------------
// MenuItem setters
// ---------------------------------------------------------------------

fn menu_item_set_title() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    // NSMenuItem::new initialises title to its class name; what
    // matters here is that set_title overwrites that to whatever
    // we pass.
    it.set_title("Hello");
    assert_eq!(it.ns_item().title().to_string(), "Hello");
}

fn menu_item_set_enabled() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    // NSMenuItem defaults to enabled=true.
    assert!(it.ns_item().isEnabled());
    it.set_enabled(false);
    assert!(!it.ns_item().isEnabled());
    it.set_enabled(true);
    assert!(it.ns_item().isEnabled());
}

fn menu_item_set_checked_toggles_state() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    assert_eq!(it.ns_item().state(), NSControlStateValueOff);
    it.set_checked(true);
    assert_eq!(it.ns_item().state(), NSControlStateValueOn);
    it.set_checked(false);
    assert_eq!(it.ns_item().state(), NSControlStateValueOff);
}

fn menu_item_set_shortcut_sets_key_and_modifiers() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    it.set_shortcut("r", Modifiers::CMD);
    assert_eq!(it.ns_item().keyEquivalent().to_string(), "r");
    let mask = it.ns_item().keyEquivalentModifierMask();
    assert!(mask.contains(NSEventModifierFlags::Command));
    assert!(!mask.contains(NSEventModifierFlags::Shift));
    assert!(!mask.contains(NSEventModifierFlags::Option));
}

fn menu_item_set_shortcut_cmd_shift() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    it.set_shortcut("z", Modifiers::CMD_SHIFT);
    assert_eq!(it.ns_item().keyEquivalent().to_string(), "z");
    let mask = it.ns_item().keyEquivalentModifierMask();
    assert!(mask.contains(NSEventModifierFlags::Command));
    assert!(mask.contains(NSEventModifierFlags::Shift));
}

fn menu_item_set_shortcut_empty_key_clears() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    it.set_shortcut("r", Modifiers::CMD);
    assert_eq!(it.ns_item().keyEquivalent().to_string(), "r");
    it.set_shortcut("", Modifiers::default());
    assert_eq!(it.ns_item().keyEquivalent().to_string(), "");
}

fn menu_separator_is_separator_item() {
    let mtm = common::test_mtm();
    let sep = menu_separator(mtm);
    assert!(sep.ns_item().isSeparatorItem());
}

fn menu_item_default_is_not_separator() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    assert!(!it.ns_item().isSeparatorItem());
}

// ---------------------------------------------------------------------
// Action wiring
// ---------------------------------------------------------------------

fn set_action_installs_target_and_action_selector() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    assert!(it.ns_item().target().is_none());
    assert!(it.ns_item().action().is_none());

    it.set_action(|| {}, mtm);

    assert!(it.ns_item().target().is_some(), "target should be set");
    assert!(it.ns_item().action().is_some(), "action selector should be set");
}

fn set_action_twice_panics() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    it.set_action(|| {}, mtm);

    // Second install should panic per the single-handler contract.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        it.set_action(|| {}, mtm);
    }));
    assert!(
        result.is_err(),
        "second set_action call should have panicked"
    );
}

fn set_action_closure_fires_when_selector_dispatched() {
    use objc2::{msg_send, runtime::AnyObject};
    use std::cell::Cell;
    use std::rc::Rc;

    let mtm = common::test_mtm();
    let it = menu_item(mtm);

    let fired = Rc::new(Cell::new(0));
    let f = fired.clone();
    it.set_action(move || f.set(f.get() + 1), mtm);

    // Synthesize the AppKit dispatch: target.actionFired(item).
    let target = it.ns_item().target().expect("target installed");
    let target_ptr: *const _ = &*target;
    let target_any: &AnyObject = unsafe { &*(target_ptr as *const AnyObject) };
    let item_obj: &objc2_app_kit::NSMenuItem = it.ns_item();
    let _: () = unsafe { msg_send![target_any, actionFired: item_obj] };

    assert_eq!(fired.get(), 1, "closure should fire exactly once");
}

// ---------------------------------------------------------------------
// Modifiers translation — exercise each flag independently
// ---------------------------------------------------------------------

fn modifiers_none_emits_empty_mask() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    it.set_shortcut("x", Modifiers::default());
    let mask = it.ns_item().keyEquivalentModifierMask();
    assert!(!mask.contains(NSEventModifierFlags::Command));
    assert!(!mask.contains(NSEventModifierFlags::Shift));
    assert!(!mask.contains(NSEventModifierFlags::Option));
    assert!(!mask.contains(NSEventModifierFlags::Control));
}

fn modifiers_all_four_emits_full_mask() {
    let mtm = common::test_mtm();
    let it = menu_item(mtm);
    let mods = Modifiers::default().command().shift().option().control();
    it.set_shortcut("x", mods);
    let mask = it.ns_item().keyEquivalentModifierMask();
    assert!(mask.contains(NSEventModifierFlags::Command));
    assert!(mask.contains(NSEventModifierFlags::Shift));
    assert!(mask.contains(NSEventModifierFlags::Option));
    assert!(mask.contains(NSEventModifierFlags::Control));
}

// ---------------------------------------------------------------------
// Test runner
// ---------------------------------------------------------------------

fn main() {
    common::run_tests(&[
        ("menu_bar_starts_empty", menu_bar_starts_empty),
        ("menu_construction_sets_title", menu_construction_sets_title),
        ("menu_set_title_updates_ns_menu_title", menu_set_title_updates_ns_menu_title),
        ("append_menu_creates_wrapper_with_submenu_pointer", append_menu_creates_wrapper_with_submenu_pointer),
        ("append_submenu_in_menu_creates_nested_wrapper", append_submenu_in_menu_creates_nested_wrapper),
        ("menu_item_set_title", menu_item_set_title),
        ("menu_item_set_enabled", menu_item_set_enabled),
        ("menu_item_set_checked_toggles_state", menu_item_set_checked_toggles_state),
        ("menu_item_set_shortcut_sets_key_and_modifiers", menu_item_set_shortcut_sets_key_and_modifiers),
        ("menu_item_set_shortcut_cmd_shift", menu_item_set_shortcut_cmd_shift),
        ("menu_item_set_shortcut_empty_key_clears", menu_item_set_shortcut_empty_key_clears),
        ("menu_separator_is_separator_item", menu_separator_is_separator_item),
        ("menu_item_default_is_not_separator", menu_item_default_is_not_separator),
        ("set_action_installs_target_and_action_selector", set_action_installs_target_and_action_selector),
        ("set_action_twice_panics", set_action_twice_panics),
        ("set_action_closure_fires_when_selector_dispatched", set_action_closure_fires_when_selector_dispatched),
        ("modifiers_none_emits_empty_mask", modifiers_none_emits_empty_mask),
        ("modifiers_all_four_emits_full_mask", modifiers_all_four_emits_full_mask),
    ]);
}
