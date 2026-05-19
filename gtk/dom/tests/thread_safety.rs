//! Tests that off-main-thread access to GTK-backed types panics.
//!
//! GTK4 demands the main thread for almost every API. gtk-rs enforces
//! this via `assert_initialized_main_thread!` — every widget setter
//! checks that GTK was initialised on the calling thread. Threads
//! other than the one that called `gtk::init` panic.

#![cfg(feature = "gtk")]

mod common;

/// Constructing a GTK widget from a non-main thread should panic
/// (via gtk-rs's main-thread assertion).
fn create_off_main_panics() {
    common::ensure_gtk_init();
    if common::is_headless() {
        return;
    }

    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            // `new_tree()` is main-thread-safe (pure Rust); the panic
            // comes from the GTK widget constructor.
            let tree = gtk_dom::layout::new_tree();
            let _ = gtk_dom::Element::create(&tree, "button");
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Element::create off main should have panicked"
    );
}

fn create_text_off_main_panics() {
    common::ensure_gtk_init();
    if common::is_headless() {
        return;
    }
    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            let tree = gtk_dom::layout::new_tree();
            let _ = gtk_dom::Element::create_text(&tree, "hi");
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Element::create_text off main should have panicked"
    );
}

fn create_placeholder_off_main_panics() {
    common::ensure_gtk_init();
    if common::is_headless() {
        return;
    }
    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            let tree = gtk_dom::layout::new_tree();
            let _ = gtk_dom::Element::create_placeholder(&tree);
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Element::create_placeholder off main should have panicked"
    );
}

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
