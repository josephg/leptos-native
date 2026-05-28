//! CoreAnimation Phase 1 demo.
//!
//! - "Fade" button toggles a panel's `alpha` over an ease-in-out
//!   curve.
//! - "Color" button cross-fades a panel's `background_color` over
//!   a spring.
//! - "Both" button uses a single `with_animation` to do both at
//!   once.

extern crate leptos_cocoa as leptos_platform;

#[cfg(target_os = "macos")]
mod app {
    use leptos_platform::cocoa::animation::{ease_in_out, spring, with_animation};
    use leptos_platform::prelude::*;

    #[component]
    pub fn Demo() -> impl IntoView {
        let visible = RwSignal::new(true);
        let warm = RwSignal::new(false);
        let scale = RwSignal::new(1.0_f64);
        // Phase 2: layout animation. Toggling `wide` changes the
        // panel's width, which triggers a Taffy relayout — the
        // resulting setFrame: calls animate when inside
        // `with_animation`.
        let wide = RwSignal::new(false);

        let alpha = move || if visible.get() { 1.0 } else { 0.2 };
        let bg = move || {
            if warm.get() {
                Color::rgb(0.95, 0.55, 0.25)
            } else {
                Color::rgb(0.25, 0.55, 0.95)
            }
        };
        let panel_w = move || Dim::px(if wide.get() { 480.0 } else { 240.0 });

        view! {
            <vstack padding=20.0 gap=16.0>
                <stack
                    alpha=alpha
                    background_color=bg
                    corner_radius=14.0
                    width=panel_w
                    height=120.0
                    scale=move || scale.get()
                />
                <hstack gap=8.0>
                    <button on:click=move |_| {
                        with_animation(ease_in_out(0.35), move || {
                            visible.update(|v| *v = !*v);
                        });
                    }>"Fade"</button>
                    <button on:click=move |_| {
                        with_animation(spring(), move || {
                            warm.update(|w| *w = !*w);
                        });
                    }>"Color"</button>
                    <button on:click=move |_| {
                        // Snap down on a snappy curve, pop back up
                        // with a bounce. Two transactions, chained
                        // by a deferred restore.
                        with_animation(ease_in_out(0.08), move || {
                            scale.set(0.92);
                        });
                        any_spawner::Executor::spawn_local(async move {
                            with_animation(spring(), move || {
                                scale.set(1.0);
                            });
                        });
                    }>"Press"</button>
                    <button on:click=move |_| {
                        with_animation(spring(), move || {
                            wide.update(|w| *w = !*w);
                        });
                    }>"Resize"</button>
                    <button on:click=move |_| {
                        with_animation(spring(), move || {
                            visible.update(|v| *v = !*v);
                            warm.update(|w| *w = !*w);
                        });
                    }>"Both"</button>
                </hstack>
            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("CoreAnimation demo", (360.0, 260.0), || {
            view! { <Demo /> }
        })
        .run();
    }
}

#[cfg(target_os = "macos")]
fn main() {
    app::main()
}

#[cfg(not(target_os = "macos"))]
fn main() {}
