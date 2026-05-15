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
    toolbar, toolbar_flexible_space, toolbar_item, toolbar_search_item,
    ToolbarMountable,
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
                    .icon(cocoa_dom::Icon::sf_symbol("plus")),
            )
            .child(
                toolbar_item()
                    .identifier("b")
                    .label("Bravo")
                    .icon(cocoa_dom::Icon::sf_symbol("minus")),
            );

        let mut state = <_ as Render<Dom>>::build(view);
        state.mount(&opened.content_root, None);

        // `nswindow.toolbar()` returning Some is the main assertion
        // — it proves the attach path ran end-to-end. Verifying
        // the visible-items list requires the AppKit run loop to
        // tick, so we test that separately via the delegate in
        // `flexible_space_renders_between_items`.
        assert!(
            opened.nswindow.toolbar().is_some(),
            "window should have a toolbar attached after mount"
        );

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
        let reg = build
            .items
            .get("clicker")
            .expect("item should be registered under its identifier");
        let target = reg.ns_item.target().expect("target must be set");
        // ActionTarget's `actionFired:` method returns void, so we
        // can't use `performSelector:withObject:` here (that path
        // expects an `id` return). Invoke the selector directly.
        unsafe {
            let sender: *const objc2_app_kit::NSToolbarItem = &*reg.ns_item;
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

        let reg = build.items.get("dyn").expect("item present");
        let before = reg.ns_item.label().to_string();
        assert_eq!(before, "initial", "initial label should be installed");

        label.set("changed".to_string());
        // Let the reactive scheduler tick so the install() effect
        // re-runs and pushes the new title into the NSToolbarItem.
        common::pump_run_loop(0.05);
        let after = reg.ns_item.label().to_string();
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

// ---------------------------------------------------------------------
// 7. <toolbar> mounted inside a split-pane reaches the window
// ---------------------------------------------------------------------

/// Pages's layout is `mount_to_split_window → split_pane → vstack →
/// toolbar`. The toolbar's `Mountable::mount` walks up from the
/// vstack to find the containing NSWindow. Verify that walk
/// resolves through the split-view-controller's view hierarchy.
fn toolbar_attaches_through_split_pane() {
    use leptos_cocoa::cocoa::element::vstack;
    use leptos_cocoa::cocoa::split::{split_pane, split_view};

    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let sv = split_view().vertical(true).child(
            split_pane().child(
                vstack().child(
                    toolbar()
                        .identifier("split-test.toolbar")
                        .child(
                            toolbar_item()
                                .identifier("first")
                                .label("First")
                                .icon(cocoa_dom::Icon::sf_symbol("plus")),
                        ),
                ),
            ),
        );
        let (opened, state) =
            sv.build_and_install("toolbar-in-split", (900.0, 600.0), mtm);

        // The split window's NSWindow should now have a toolbar
        // attached — set during the toolbar's Mountable::mount
        // via the parent's `.window()` walk.
        let ns_toolbar = opened
            .nswindow
            .toolbar()
            .expect("split-window should have a toolbar after mount");
        assert_eq!(
            ns_toolbar.identifier().to_string(),
            "split-test.toolbar",
        );

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 8. ToolbarHandle: insert + remove items after build
// ---------------------------------------------------------------------

fn toolbar_handle_insert_and_remove() {
    use cocoa_dom::window::open_window;
    use leptos_cocoa::cocoa::toolbar::ToolbarHandle;

    let mtm = common::test_mtm();
    with_reactive_scope(|| {
        let opened = open_window("handle-test", (640.0, 480.0), mtm);
        let handle = ToolbarHandle::new();

        let view = toolbar()
            .handle(handle)
            .child(toolbar_item().identifier("first").label("First"));
        let mut state = <_ as Render<Dom>>::build(view);
        state.mount(&opened.content_root, None);

        // Insert a second item at index 1 (after "first").
        let inserted = handle.insert_item(
            toolbar_item().identifier("second").label("Second"),
            1,
        );
        assert!(inserted, "insert_item should succeed for a fresh identifier");
        assert!(handle.contains_item("first"));
        assert!(handle.contains_item("second"));

        // Duplicate insert should report failure.
        let duplicate = handle.insert_item(
            toolbar_item().identifier("first").label("Dupe"),
            0,
        );
        assert!(!duplicate, "duplicate identifier insertion should fail");

        // Remove the second one.
        let removed = handle.remove_item("second");
        assert!(removed);
        assert!(!handle.contains_item("second"));

        // Removing a non-existent identifier is a no-op.
        assert!(!handle.remove_item("never_added"));

        std::mem::forget(state);
        std::mem::forget(opened);
    });
}

// ---------------------------------------------------------------------
// 9. <toolbar_search_item> with bind:value updates the search field
// ---------------------------------------------------------------------

fn search_item_bind_value_round_trips() {
    use leptos_cocoa::cocoa::bind::BindAttribute;
    use leptos_cocoa::cocoa::toolbar::ToolbarBuild;

    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let mut build = ToolbarBuild::new_for_test();

        let q = RwSignal::new(String::from("initial"));
        let item = toolbar_search_item()
            .identifier("search")
            .placeholder("Type to search…")
            .bind(leptos_cocoa::attr::Value, q);
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        // The registration carries a search Element wrapping the
        // embedded NSSearchField.
        let reg = build.items.get("search").expect("registered");
        let el = reg
            .search_element
            .as_ref()
            .expect("search_element populated for search items");

        // Initial value should propagate into the field via the
        // install_text_field_value_bind RenderEffect.
        common::pump_run_loop(0.05);
        use objc2_app_kit::NSControl;
        let field: &NSControl = unsafe {
            &*(el.ns_view() as *const _ as *const NSControl)
        };
        assert_eq!(field.stringValue().to_string(), "initial");

        // Updating the signal pushes a new value into the field.
        q.set("updated".to_string());
        common::pump_run_loop(0.05);
        assert_eq!(field.stringValue().to_string(), "updated");
    });
}

// ---------------------------------------------------------------------
// 10. icon=Icon::SfSymbol(...) sets the NSToolbarItem image
// ---------------------------------------------------------------------

fn icon_sf_symbol_sets_image() {
    use cocoa_dom::Icon;
    use leptos_cocoa::cocoa::toolbar::ToolbarBuild;

    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let mut build = ToolbarBuild::new_for_test();
        let item = toolbar_item()
            .identifier("sym")
            .label("Sym")
            .icon(Icon::sf_symbol("plus"));
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        let reg = build.items.get("sym").expect("registered");
        assert!(
            reg.ns_item.image().is_some(),
            "SF Symbol icon should populate NSToolbarItem.image"
        );
    });
}

// ---------------------------------------------------------------------
// 11. icon=Icon::Image("") clears the image
// ---------------------------------------------------------------------

fn icon_empty_image_path_clears() {
    use cocoa_dom::Icon;
    use leptos_cocoa::cocoa::toolbar::ToolbarBuild;

    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let mut build = ToolbarBuild::new_for_test();
        let item = toolbar_item()
            .identifier("empty")
            .label("Empty")
            .icon(Icon::image(""));
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        let reg = build.items.get("empty").expect("registered");
        assert!(
            reg.ns_item.image().is_none(),
            "Icon::image(empty) should leave the image slot None"
        );
    });
}

// ---------------------------------------------------------------------
// 12. Variant transitions: SfSymbol → Image → SfSymbol → None
//
// This is the bug the unified-Icon refactor exists to prevent:
// when the icon flips from an SF Symbol to a file image, the
// previous SF Symbol's diff state must clear so AppKit only ever
// has one image source active. The unified `last_icon` cell makes
// the invariant explicit — the underlying NSImage pointer changes
// on every real transition, and stays put on no-op re-emission.
// ---------------------------------------------------------------------

fn icon_variant_transitions_replace_atomically() {
    use cocoa_dom::Icon;
    use leptos_cocoa::cocoa::toolbar::ToolbarBuild;

    let _mtm = common::test_mtm();
    with_reactive_scope(|| {
        let mut build = ToolbarBuild::new_for_test();

        // Drive the icon from a signal so we can flip variants
        // mid-test and observe the resulting NSImage changes.
        let icon_sig = RwSignal::new(Icon::sf_symbol("plus"));
        let item = toolbar_item()
            .identifier("trans")
            .label("Trans")
            .icon(move || icon_sig.get());
        let _state = item.build_into_toolbar(&mut build, common::test_mtm());

        let reg = build.items.get("trans").expect("registered");

        // Initial state: SF Symbol → NSImage populated.
        let img_after_sf = reg.ns_item.image();
        assert!(
            img_after_sf.is_some(),
            "initial SfSymbol should populate image"
        );

        // Transition: SfSymbol → Image (empty path → clears).
        icon_sig.set(Icon::image(""));
        common::pump_run_loop(0.05);
        assert!(
            reg.ns_item.image().is_none(),
            "transitioning to Icon::image(empty) should clear the image"
        );

        // Transition: Image → SfSymbol — image re-populated.
        icon_sig.set(Icon::sf_symbol("minus"));
        common::pump_run_loop(0.05);
        assert!(
            reg.ns_item.image().is_some(),
            "transitioning back to a valid SF Symbol should re-populate"
        );

        // No-op: re-emit the SAME SF Symbol. The image pointer
        // should not change — this is what the top-level diff in
        // `set_icon` guarantees.
        let img_before_redundant = reg.ns_item.image();
        icon_sig.set(Icon::sf_symbol("minus"));
        common::pump_run_loop(0.05);
        let img_after_redundant = reg.ns_item.image();
        let same_ptr = match (&img_before_redundant, &img_after_redundant) {
            (Some(a), Some(b)) => {
                let a: *const objc2_app_kit::NSImage = &**a;
                let b: *const objc2_app_kit::NSImage = &**b;
                a == b
            }
            (None, None) => true,
            _ => false,
        };
        assert!(
            same_ptr,
            "re-emitting the same Icon should be a no-op (diff bails); \
             a fresh NSImage pointer indicates the diff didn't fire"
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
        (
            "toolbar_attaches_through_split_pane",
            toolbar_attaches_through_split_pane,
        ),
        (
            "toolbar_handle_insert_and_remove",
            toolbar_handle_insert_and_remove,
        ),
        (
            "search_item_bind_value_round_trips",
            search_item_bind_value_round_trips,
        ),
        ("icon_sf_symbol_sets_image", icon_sf_symbol_sets_image),
        (
            "icon_empty_image_path_clears",
            icon_empty_image_path_clears,
        ),
        (
            "icon_variant_transitions_replace_atomically",
            icon_variant_transitions_replace_atomically,
        ),
    ]);
}
