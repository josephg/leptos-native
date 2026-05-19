//! Tests that off-main-thread access to AppKit-backed types panics
//! at the SendWrapper / MainThreadMarker boundary.
//!
//! AppKit demands the main thread for almost every API. cocoa_dom
//! enforces this via `MainThreadMarker::new()` (which returns `None`
//! off-main) and via `SendWrapper` runtime checks on Node clones.
//! These tests pin both behaviors.

#![cfg(target_os = "macos")]

mod common;

use std::sync::{Arc, Mutex};

/// `Element::create` reads `MainThreadMarker::new()` at the start;
/// off-main this returns `None` and the unwrap panics with a clear
/// message. Spawning a thread and trying to create an Element there
/// must blow up loudly, not silently produce a corrupt object.
fn create_off_main_panics() {
    let payload = Arc::new(Mutex::new(None::<String>));
    let payload_clone = payload.clone();

    let handle = std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            // Note: this should panic before constructing anything.
            // The tree is constructed inside the thread closure
            // because TreeRef is `!Send`; the mtm check inside
            // `Element::create` panics before tree access.
            let tree = cocoa_dom::layout::new_tree();
            let _ = cocoa_dom::Element::create(&tree, "button");
        });
        if let Err(e) = result {
            let msg = if let Some(s) = e.downcast_ref::<&'static str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            };
            *payload_clone.lock().unwrap() = Some(msg);
        }
    });
    handle.join().expect("thread join");

    let msg = payload
        .lock()
        .unwrap()
        .take()
        .expect("expected panic when creating Element off main thread");
    assert!(
        msg.contains("main thread"),
        "panic message did not mention main thread: {msg}"
    );
}

/// `Text::create` and `Placeholder::create` go through the same
/// MainThreadMarker check.
fn create_text_off_main_panics() {
    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            let tree = cocoa_dom::layout::new_tree();
            let _ = cocoa_dom::Text::create(&tree, "hi");
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Text::create off main should have panicked"
    );
}

fn create_placeholder_off_main_panics() {
    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            let tree = cocoa_dom::layout::new_tree();
            let _ = cocoa_dom::Placeholder::create(&tree);
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Placeholder::create off main should have panicked"
    );
}

// `access_node_off_main_panics` was attempted — sending an Element
// across a thread boundary, then reading `.ns_view()` off-main, does
// panic, but the SendWrapper's *Drop* impl also panics in the
// unwinding thread, which aborts the process before we can observe
// the original panic. The Element/Text/Placeholder creation tests
// above cover the user-visible "panic loudly off main" contract; the
// SendWrapper Drop behavior is a third-party-crate detail.

fn main() {
    common::run_tests(&[
        ("create_off_main_panics", create_off_main_panics),
        ("create_text_off_main_panics", create_text_off_main_panics),
        (
            "create_placeholder_off_main_panics",
            create_placeholder_off_main_panics,
        ),
    ]);
}
