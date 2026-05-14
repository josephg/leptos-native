//! Tests for the native `<toolbar>` + `<toolbar_item>` elements.
//!
//! These exercise the leptos_cocoa builders against real
//! `NSToolbar` / `NSToolbarItem` / `NSToolbarDelegate` instances
//! (no UI loop — just object construction + state inspection).

#![cfg(target_os = "macos")]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cocoa_dom::window::open_window;
use leptos_cocoa::cocoa::toolbar::{
    toolbar, toolbar_flexible_space, toolbar_item, ToolbarMountable,
};
use leptos_cocoa::Dom;
use reactive_graph::{
    owner::Owner,
    signal::RwSignal,
    traits::{Get, Set},
};
use renderer::view::{Mountable, Render};

fn with_reactive_scope<F: FnOnce()>(body: F) {
    let _ = cocoa_dom::spawner::init();
    let owner = Owner::new();
    owner.with(body);
}

// ---------------------------------------------------------------------
// 1. <toolbar> attaches to its containing NSWindow
// ---------------------------------------------------------------------

fn toolbar_attaches_to_window() {
    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let opened = open_window("toolbar-attach", (640.0, 480.0), mtm);

        // Build a <toolbar> with two items.
        let view = toolbar()
            .identifier("test.toolbar")
            .child(
                toolbar_item()
                    .identifier("a")
                    .label("Alpha")
                    .sf_symbol("plus"),
            )
            .child(
                toolbar_item()
                    .identifier("b")
                    .label("Bravo")
                    .sf_symbol("minus"),
            );

        let mut state = <_ as Render<Dom>>::build(view);
        state.mount(&opened.content_root, None);

        // The window should now have a toolbar attached.
        let ns_toolbar = opened
            .nswindow
            .toolbar()
            .expect("window should have a toolbar after mount");

        // `nswindow.toolbar()` returning Some is the main assertion
        // — it proves the attach path ran end-to-end. Verifying the
        // visible-items list requires the AppKit run loop to tick,
        // so we test that separately via the delegate in
        // `flexible_space_renders_between_items`.

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 2. duplicate identifier panics at build time
// ---------------------------------------------------------------------

fn duplicate_identifier_panics() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = toolbar()
            .child(
                toolbar_item()
                    .identifier("dup")
                    .label("First"),
            )
            .child(
                toolbar_item()
                    .identifier("dup")
                    .label("Second"),
            );
        let result = std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(|| {
                let _ = <_ as Render<Dom>>::build(view);
            }),
        );
        assert!(
            result.is_err(),
            "two toolbar_items with the same identifier should panic"
        );
    });
}

// ---------------------------------------------------------------------
// 3. <toolbar_item> action fires when invoked
// ---------------------------------------------------------------------

fn toolbar_item_action_fires() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        // Build a single item via the ToolbarMountable cascade
        // directly so we can grab its NSToolbarItem afterwards.
        use leptos_cocoa::cocoa::toolbar::ToolbarBuild;
        let mut build = ToolbarBuild::new_for_test();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        let item = toolbar_item()
            .identifier("clicker")
            .label("Click me")
            .on(leptos_cocoa::event_macos::action, move |_| {
                fired_clone.store(true, Ordering::SeqCst);
            });
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        // Pull the NSToolbarItem back out of the build map and
        // invoke its target/action manually.
        let ns_item = build
            .items
            .get("clicker")
            .expect("item should be registered under its identifier");
        let target = ns_item.target().expect("target must be set");
        // ActionTarget's `actionFired:` method returns void, so we
        // can't use `performSelector:withObject:` here (that path
        // expects an `id` return). Invoke the selector directly.
        unsafe {
            let sender: *const objc2_app_kit::NSToolbarItem = &**ns_item;
            let _: () = objc2::msg_send![&*target, actionFired: sender];
        }

        assert!(
            fired.load(Ordering::SeqCst),
            "on:action handler should have fired"
        );
    });
}

// ---------------------------------------------------------------------
// 4. reactive label updates the underlying item
// ---------------------------------------------------------------------

fn reactive_label_updates_item_title() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use leptos_cocoa::cocoa::toolbar::ToolbarBuild;
        let mut build = ToolbarBuild::new_for_test();

        let label = RwSignal::new(String::from("initial"));
        let item = toolbar_item()
            .identifier("dyn")
            .label(move || label.get());
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        let ns_item = build.items.get("dyn").expect("item present");
        let before = ns_item.label().to_string();
        assert_eq!(before, "initial", "initial label should be installed");

        label.set("changed".to_string());
        // Let the reactive scheduler tick so the install() effect
        // re-runs and pushes the new title into the NSToolbarItem.
        common::pump_run_loop(0.05);
        let after = ns_item.label().to_string();
        assert_eq!(
            after, "changed",
            "label should track the signal reactively"
        );
    });
}

// ---------------------------------------------------------------------
// 5. flexible_space identifier appears between items
// ---------------------------------------------------------------------

fn flexible_space_renders_between_items() {
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        use leptos_cocoa::cocoa::toolbar::ToolbarBuild;
        let mut build = ToolbarBuild::new_for_test();

        let view = (
            toolbar_item().identifier("a").label("A"),
            toolbar_flexible_space(),
            toolbar_item().identifier("b").label("B"),
        );
        let _state = view.build_into_toolbar(&mut build, common::test_mtm());

        assert_eq!(build.ordered.len(), 3, "three identifiers in order");
        assert_eq!(build.ordered[0], "a");
        assert_eq!(
            build.ordered[1],
            cocoa_dom::toolbar::flexible_space_identifier(),
            "middle slot is AppKit's flexible-space identifier"
        );
        assert_eq!(build.ordered[2], "b");

        // Custom items map populated for a and b; flexible space
        // is NOT in the map (AppKit vends it directly).
        assert_eq!(build.items.len(), 2);
        assert!(build.items.contains_key("a"));
        assert!(build.items.contains_key("b"));
    });
}

// ---------------------------------------------------------------------
// 6. dropping the Toolbar releases its action handlers
// ---------------------------------------------------------------------

fn drop_releases_action_target() {
    use cocoa_dom::event;
    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let view = toolbar().child(
            toolbar_item()
                .identifier("drop")
                .label("Drop me")
                .on(leptos_cocoa::event_macos::action, |_| {}),
        );
        let state = <_ as Render<Dom>>::build(view);

        // Find the handler-store key by scanning the inner
        // Toolbar's handler_keys. The state's Toolbar holds it.
        let key = state.handler_key_for_test(0);
        assert!(
            event::handler_count_for_test_key(key) > 0,
            "action handler should be retained before drop"
        );

        drop(state);

        assert_eq!(
            event::handler_count_for_test_key(key),
            0,
            "action handler should be released after Toolbar is dropped"
        );
    });
}

fn main() {
    common::run_tests(&[
        ("toolbar_attaches_to_window", toolbar_attaches_to_window),
        ("duplicate_identifier_panics", duplicate_identifier_panics),
        ("toolbar_item_action_fires", toolbar_item_action_fires),
        (
            "reactive_label_updates_item_title",
            reactive_label_updates_item_title,
        ),
        (
            "flexible_space_renders_between_items",
            flexible_space_renders_between_items,
        ),
        ("drop_releases_action_target", drop_releases_action_target),
    ]);
}
