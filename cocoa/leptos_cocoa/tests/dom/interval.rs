//! Tests for `set_interval_with_handle` — fires repeatedly,
//! `clear()` actually stops it.

#![cfg(target_os = "macos")]

mod common;

use leptos_cocoa::dom::set_interval_with_handle;
use std::{
    cell::Cell,
    rc::Rc,
    time::Duration,
};

fn fires_repeatedly_until_cleared() {
    let _mtm = common::test_mtm();
    let count = Rc::new(Cell::new(0u32));
    let handle = {
        let c = count.clone();
        set_interval_with_handle(
            move || c.set(c.get() + 1),
            Duration::from_millis(30),
        )
        .expect("schedule")
    };

    // 200ms / 30ms = ~6 ticks expected. Pump the run loop for
    // a bit longer than that.
    common::pump_run_loop(0.25);
    let after_first_pump = count.get();
    assert!(
        after_first_pump >= 3,
        "expected at least 3 ticks in 250ms; got {}",
        after_first_pump
    );

    // Cancel and pump some more — count must NOT keep growing.
    handle.clear();
    common::pump_run_loop(0.15);
    let after_clear = count.get();
    assert_eq!(
        after_clear, after_first_pump,
        "count should not advance after clear()"
    );
}

fn clear_is_idempotent() {
    let _mtm = common::test_mtm();
    let count = Rc::new(Cell::new(0u32));
    let handle = {
        let c = count.clone();
        set_interval_with_handle(
            move || c.set(c.get() + 1),
            Duration::from_millis(30),
        )
        .expect("schedule")
    };
    handle.clear();
    handle.clear(); // safe to repeat
    common::pump_run_loop(0.1);
    assert_eq!(count.get(), 0, "no ticks after immediate clear");
}

fn main() {
    common::run_tests(&[
        ("fires_repeatedly_until_cleared", fires_repeatedly_until_cleared),
        ("clear_is_idempotent", clear_is_idempotent),
    ]);
}
