//! Builder-style API (no `view!` macro) on macOS.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;
    use leptos::tachys::html::{
        element::{button, hstack, label, vstack},
        event::click,
    };

    #[derive(Debug, Clone)]
    pub struct Count {
        value: i32,
        step: i32,
    }

    pub fn main() {
        let count = RwSignal::new(Count::new(0, 1));

        let view = vstack().padding(16.0).gap(12.0).child((
            label().child(move || format!("Count: {}", count.get().value())),
            hstack().gap(8.0).child((
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
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
