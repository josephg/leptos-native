//! Tests for the `AttributeKey` marker types.

use leptos_apple_shared::attr_keys::{AttributeKey, Checked, Value};

#[test]
fn value_key_name() {
    assert_eq!(Value::KEY, "value");
}

#[test]
fn checked_key_name() {
    assert_eq!(Checked::KEY, "checked");
}

#[test]
fn keys_are_send_static() {
    fn assert_send_static<T: Send + 'static>() {}
    assert_send_static::<Value>();
    assert_send_static::<Checked>();
}

#[test]
fn keys_are_clone() {
    let v = Value;
    let _ = v.clone();
    let c = Checked;
    let _ = c.clone();
}

#[test]
fn keys_are_zero_sized() {
    use std::mem::size_of;
    assert_eq!(size_of::<Value>(), 0);
    assert_eq!(size_of::<Checked>(), 0);
}
