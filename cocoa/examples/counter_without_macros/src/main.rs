//! Builder-style API (no `view!` macro) on macOS.

use leptos::prelude::*;
use leptos::tachys::{
    cocoa::element::{button, label, vstack},
    html::event::click,
};

#[derive(Debug, Clone)]
struct Count {
    value: i32,
    step: i32,
}

fn main() {
    let count = RwSignal::new(Count::new(0, 1));

    let view = vstack().padding(16.0).gap(12.0).child((
        label().child(move || format!("Count: {}", count.get().value())),
        // hstack-equivalent via tachys builder — use flex direction
        leptos::tachys::cocoa::element::hstack().gap(8.0).child((
            button()
                .on(click, move |_| count.update(Count::clear))
                .child("Clear"),
            button()
                .on(click, move |_| count.update(Count::decrease))
                .child("-1"),
            button()
                .on(click, move |_| count.update(Count::increase))
                .child("+1"),
        )),
    ));

    mount_to_window("Builder-style counter", (340.0, 200.0), move || {
        view
    });
}

impl Count {
    fn new(value: i32, step: u32) -> Self {
        Count { value, step: step as i32 }
    }

    fn value(&self) -> i32 { self.value }

    fn increase(&mut self) { self.value += self.step; }
    fn decrease(&mut self) { self.value -= self.step; }
    fn clear(&mut self) { self.value = 0; }
}
