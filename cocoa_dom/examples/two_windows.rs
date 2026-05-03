//! Multi-window demo: two independent windows, each with its own
//! state, sharing nothing.
//!
//! Validates the multi-window refactor: each `Window` opens its own
//! NSWindow and owns its own TaffyTree, fully isolated. Quitting
//! either window or Cmd-Q quits the app.
//!
//! Run with:
//!     cargo run -p cocoa_dom --example two_windows

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

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = init_app(mtm);

    let owner = Owner::new();
    owner.set();
    std::mem::forget(owner);

    // Two independent counters — each window has its own state.
    let main_count = RwSignal::new(0_i32);
    let inspector_count = RwSignal::new(100_i32);

    let view = (
        // Window 1 — "Main"
        window()
            .title("Main")
            .size(360.0, 200.0)
            .child(
                stack_view()
                    .padding(16.0)
                    .gap(12.0)
                    .child(label().text(move || {
                        format!("Main count: {}", main_count.get())
                    }))
                    .child(
                        view()
                            .flex_direction(FlexDirection::Row)
                            .gap(8.0)
                            .child(
                                button()
                                    .title("-1")
                                    .flex_grow(1.0)
                                    .on_click(move || {
                                        main_count.update(|n| *n -= 1)
                                    }),
                            )
                            .child(
                                button()
                                    .title("+1")
                                    .flex_grow(1.0)
                                    .on_click(move || {
                                        main_count.update(|n| *n += 1)
                                    }),
                            ),
                    ),
            ),
        // Window 2 — "Inspector"
        window()
            .title("Inspector")
            .size(280.0, 160.0)
            .child(
                stack_view()
                    .padding(12.0)
                    .gap(8.0)
                    .child(label().text(move || {
                        format!("Inspector: {}", inspector_count.get())
                    }))
                    .child(
                        button()
                            .title("Reset to 100")
                            .flex_grow(1.0)
                            .on_click(move || inspector_count.set(100)),
                    )
                    .child(
                        button()
                            .title("Add 10")
                            .flex_grow(1.0)
                            .on_click(move || {
                                inspector_count.update(|n| *n += 10)
                            }),
                    ),
            ),
    );

    let state = view.build();
    std::mem::forget(state);

    run_loop(&app);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cocoa_dom only runs on macOS");
}
