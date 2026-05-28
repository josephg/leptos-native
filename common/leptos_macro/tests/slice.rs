//! `slice!` macro tests. Ported from upstream
//! `leptos-upstream/leptos_macro/tests/slice.rs`.

extern crate leptos_native as leptos_platform;

use leptos_native::reactive::signal::RwSignal;
use leptos_macro::slice;

#[derive(Default)]
pub struct OuterState {
    count: i32,
    inner: InnerState,
}

#[derive(Clone, PartialEq, Default)]
pub struct InnerState {
    inner_count: i32,
    inner_tuple: InnerTuple,
}

#[derive(Clone, PartialEq, Default)]
pub struct InnerTuple(String);

#[test]
fn green() {
    let outer_signal = RwSignal::new(OuterState::default());

    let (_, _) = slice!(outer_signal.count);
    let (_, _) = slice!(outer_signal.inner.inner_count);
    let (_, _) = slice!(outer_signal.inner.inner_tuple.0);
}
