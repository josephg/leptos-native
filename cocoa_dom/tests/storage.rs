//! Tests for `local_storage()` / `Storage` (NSUserDefaults wrapper).
//!
//! Each test uses a unique key prefix so concurrent / re-runs
//! don't collide via the persistent global NSUserDefaults store
//! (which keeps state across runs).

#![cfg(target_os = "macos")]

mod common;

use cocoa_dom::local_storage;

/// Build a per-test key prefix to avoid collisions with other tests
/// or prior test runs (NSUserDefaults is persistent across the
/// whole test binary's lifetime — and across processes).
fn unique_key(suffix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("cocoa_dom_test.{}.{}", nanos, suffix)
}

fn local_storage_returns_some_storage() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("present");
    let _ = s.set_item(&key, "x");
    let _ = s.remove_item(&key);
}

fn set_then_get_round_trip() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("roundtrip");

    s.set_item(&key, "hello").unwrap();
    let v = s.get_item(&key).unwrap();
    assert_eq!(v.as_deref(), Some("hello"));

    s.remove_item(&key).unwrap();
}

fn get_missing_key_returns_none() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("missing");
    let v = s.get_item(&key).unwrap();
    assert_eq!(v, None);
}

fn remove_item_actually_removes() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("remove");

    s.set_item(&key, "v").unwrap();
    assert_eq!(s.get_item(&key).unwrap().as_deref(), Some("v"));

    s.remove_item(&key).unwrap();
    assert_eq!(s.get_item(&key).unwrap(), None);
}

fn set_item_overwrites() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("overwrite");

    s.set_item(&key, "first").unwrap();
    s.set_item(&key, "second").unwrap();
    assert_eq!(
        s.get_item(&key).unwrap().as_deref(),
        Some("second")
    );

    s.remove_item(&key).unwrap();
}

fn json_round_trip_via_string() {
    // Realistic todomvc-style: store JSON-serialized data as a
    // single string under one key.
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("json");

    let payload =
        r#"{"items":[{"id":1,"label":"buy milk","done":false}]}"#;
    s.set_item(&key, payload).unwrap();
    let read = s.get_item(&key).unwrap().unwrap();
    assert_eq!(read, payload);

    s.remove_item(&key).unwrap();
}

fn unicode_keys_and_values() {
    let _mtm = common::test_mtm();
    let s = local_storage().expect("ok").expect("Some");
    let key = unique_key("unicode-🦀");
    s.set_item(&key, "résumé — 日本語 — 🎉").unwrap();
    assert_eq!(
        s.get_item(&key).unwrap().as_deref(),
        Some("résumé — 日本語 — 🎉")
    );
    s.remove_item(&key).unwrap();
}

fn main() {
    common::run_tests(&[
        ("local_storage_returns_some_storage", local_storage_returns_some_storage),
        ("set_then_get_round_trip", set_then_get_round_trip),
        ("get_missing_key_returns_none", get_missing_key_returns_none),
        ("remove_item_actually_removes", remove_item_actually_removes),
        ("set_item_overwrites", set_item_overwrites),
        ("json_round_trip_via_string", json_round_trip_via_string),
        ("unicode_keys_and_values", unicode_keys_and_values),
    ]);
}
