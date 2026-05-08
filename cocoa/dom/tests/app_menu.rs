//! Tests for the menu bar that `init_app` installs:
//! App > Quit (⌘Q) and Edit > Undo / Redo / Cut / Copy / Paste /
//! Delete / Select All.

#![cfg(target_os = "macos")]

mod common;

use objc2::sel;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenuItem};

/// Idempotent: running app tests in sequence calls `init_app`
/// many times against the same NSApplication. AppKit's
/// `setMainMenu:` replaces; safe to repeat.
fn ensure_app_initialized() -> objc2::rc::Retained<NSApplication> {
    let mtm = common::test_mtm();
    cocoa_dom::app::init_app(mtm)
}

fn item(menu: &objc2_app_kit::NSMenu, idx: usize) -> objc2::rc::Retained<NSMenuItem> {
    let items = menu.itemArray();
    let v: Vec<_> = items.iter().collect();
    v[idx].clone()
}

// ---------------------------------------------------------------------
// Top-level menu structure
// ---------------------------------------------------------------------

fn main_menu_has_app_and_edit_submenus() {
    let app = ensure_app_initialized();
    let main_menu =
        app.mainMenu().expect("init_app should set the main menu");

    let items = main_menu.itemArray();
    assert!(
        items.len() >= 2,
        "main menu should have at least App + Edit submenus"
    );
}

// ---------------------------------------------------------------------
// App menu — Quit item
// ---------------------------------------------------------------------

fn app_menu_quit_has_cmd_q() {
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let app_item = item(&main_menu, 0);
    let app_menu = app_item.submenu().expect("App submenu missing");

    // App submenu's first item is Quit.
    let quit = item(&app_menu, 0);
    assert_eq!(
        quit.action(),
        Some(sel!(terminate:)),
        "Quit should send terminate:"
    );
    assert_eq!(
        quit.keyEquivalent().to_string(), "q",
        "Quit's key equivalent is q"
    );
}

// ---------------------------------------------------------------------
// Edit menu — selectors + key equivalents
// ---------------------------------------------------------------------

fn edit_menu_present_with_title() {
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let edit_item = item(&main_menu, 1);
    let edit_menu = edit_item.submenu().expect("Edit submenu missing");
    assert_eq!(edit_menu.title().to_string(), "Edit");
}

fn edit_menu_has_expected_items() {
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let edit = item(&main_menu, 1).submenu().unwrap();

    // (title or separator, expected selector or None)
    let expected: &[(&str, Option<objc2::runtime::Sel>)] = &[
        ("Undo", Some(sel!(undo:))),
        ("Redo", Some(sel!(redo:))),
        ("", None),                    // separator
        ("Cut", Some(sel!(cut:))),
        ("Copy", Some(sel!(copy:))),
        ("Paste", Some(sel!(paste:))),
        ("Delete", Some(sel!(delete:))),
        ("Select All", Some(sel!(selectAll:))),
    ];

    let items = edit.itemArray();
    let actual: Vec<_> = items.iter().collect();
    assert_eq!(
        actual.len(), expected.len(),
        "Edit menu item count mismatch"
    );

    for (i, (title, sel)) in expected.iter().enumerate() {
        let it = &actual[i];
        if it.isSeparatorItem() {
            assert!(sel.is_none(), "expected non-separator at idx {}", i);
            continue;
        }
        assert_eq!(
            it.title().to_string(), *title,
            "Edit item {} title mismatch", i
        );
        assert_eq!(
            it.action(), *sel,
            "Edit item {} selector mismatch", i
        );
    }
}

fn edit_menu_redo_has_shift_cmd_z() {
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let edit = item(&main_menu, 1).submenu().unwrap();
    // Redo is the second item.
    let redo = item(&edit, 1);
    assert_eq!(redo.title().to_string(), "Redo");
    assert_eq!(redo.keyEquivalent().to_string(), "z");
    let mods = redo.keyEquivalentModifierMask();
    assert!(mods.contains(NSEventModifierFlags::Command));
    assert!(mods.contains(NSEventModifierFlags::Shift));
}

fn edit_menu_select_all_is_cmd_a() {
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let edit = item(&main_menu, 1).submenu().unwrap();
    // Locate by title — index varies with menu order.
    let items = edit.itemArray();
    let select_all = items
        .iter()
        .find(|it| it.title().to_string() == "Select All")
        .expect("Select All item missing");
    assert_eq!(select_all.action(), Some(sel!(selectAll:)));
    assert_eq!(select_all.keyEquivalent().to_string(), "a");
}

fn edit_menu_items_have_nil_target() {
    // First-responder dispatch: target=nil means AppKit walks the
    // responder chain. NSTextField responds natively. If any Edit
    // item has a non-nil target, the shortcuts won't reach the
    // focused field.
    let app = ensure_app_initialized();
    let main_menu = app.mainMenu().unwrap();
    let edit = item(&main_menu, 1).submenu().unwrap();
    for it in edit.itemArray().iter() {
        if it.isSeparatorItem() { continue }
        assert!(
            it.target().is_none(),
            "Edit item \"{}\" should have nil target",
            it.title().to_string()
        );
    }
}

fn main() {
    common::run_tests(&[
        ("main_menu_has_app_and_edit_submenus", main_menu_has_app_and_edit_submenus),
        ("app_menu_quit_has_cmd_q", app_menu_quit_has_cmd_q),
        ("edit_menu_present_with_title", edit_menu_present_with_title),
        ("edit_menu_has_expected_items", edit_menu_has_expected_items),
        ("edit_menu_redo_has_shift_cmd_z", edit_menu_redo_has_shift_cmd_z),
        ("edit_menu_select_all_is_cmd_a", edit_menu_select_all_is_cmd_a),
        ("edit_menu_items_have_nil_target", edit_menu_items_have_nil_target),
    ]);
}
