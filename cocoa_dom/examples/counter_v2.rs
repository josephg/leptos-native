//! Counter built with the tachys::cocoa builder API + the new
//! Window Render type. Cocoa_dom's example dir, but uses the
//! tachys façade.
//!
//! For the leptos-prelude version see `examples/counter_macos`.
//!
//! Run with:
//!     cargo run -p cocoa_dom --example counter_v2

#[cfg(target_os = "macos")]
fn main() {
    use cocoa_dom::{
        app::{init_app, run_loop},
        MainThreadMarker,
    };
    use reactive_graph::{owner::Owner, signal::RwSignal, traits::*};
    use tachys::{
        cocoa::{
            element::{button, label, stack_view, view},
            window::window,
            FlexDirection,
        },
        view::Render,
    };

    let mtm = MainThreadMarker::new().expect("must run on main thread");
    let app = init_app(mtm);

    let owner = Owner::new();
    owner.set();
    std::mem::forget(owner);

    let count = RwSignal::new(0_i32);

    let view = window()
        .title("counter — Window builder")
        .size(320.0, 200.0)
        .child(
            stack_view()
                .padding(16.0)
                .gap(12.0)
                .child(label().text(move || {
                    let c = count.get();
                    eprintln!("[render] count = {c}");
                    format!("Count: {c}")
                }))
                .child(
                    view()
                        .flex_direction(FlexDirection::Row)
                        .gap(8.0)
                        .child(
                            button()
                                .title("-1")
                                .flex_grow(1.0)
                                .on_click(move || count.update(|n| *n -= 1)),
                        )
                        .child(
                            button()
                                .title("Reset")
                                .flex_grow(1.0)
                                .on_click(move || count.set(0)),
                        )
                        .child(
                            button()
                                .title("+1")
                                .flex_grow(1.0)
                                .on_click(move || count.update(|n| *n += 1)),
                        ),
                ),
        );

    // Window::build opens the NSWindow, builds children, lays out,
    // shows it. Leak the state so it survives the run loop.
    let state = view.build();
    std::mem::forget(state);

    run_loop(&app);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cocoa_dom only runs on macOS");
}
