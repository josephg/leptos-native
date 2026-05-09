//! Tests that off-main-thread access to GTK-backed types panics.
//!
//! GTK4 demands the main thread for almost every API. gtk-rs enforces
//! this via `assert_initialized_main_thread!` — every widget setter
//! checks that GTK was initialised on the calling thread. Threads
//! other than the one that called `gtk::init` panic.

#![cfg(target_os = "linux")]

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
            let _ = gtk_dom::Element::create("button");
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
            let _ = gtk_dom::Text::create("hi");
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
    common::ensure_gtk_init();
    if common::is_headless() {
        return;
    }
    let result = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            let _ = gtk_dom::Placeholder::create();
        })
    })
    .join()
    .expect("thread join");
    assert!(
        result.is_err(),
        "Placeholder::create off main should have panicked"
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
