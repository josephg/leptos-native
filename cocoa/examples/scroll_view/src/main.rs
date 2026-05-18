//! `<scroll_view>` smoke tests, one per axis mode:
//!
//! - **Vertical** (top): an unbounded list of rows. Originally the
//!   only kind of scrolling we supported.
//! - **Horizontal** (middle): a short strip of fixed-size colored
//!   rectangles totalling wider than the viewport — exercises the
//!   `axis=ScrollAxis::Horizontal` case.
//! - **Both** (bottom): a small grid of squares laid out wider AND
//!   taller than the viewport. `axis=ScrollAxis::Both` lets the
//!   user scroll on both axes.

#[cfg(target_os = "macos")]
mod app {
    use leptos::prelude::*;

    fn rect(color: Color, w: f32, h: f32, label_text: String) -> impl IntoView {
        view! {
            <vstack
                width=w
                height=h
                background_color=color
                corner_radius=4.0
                justify_content=JustifyContent::Center
                align=AlignItems::Center
            >
                <label text_color=Color::WHITE>{label_text}</label>
            </vstack>
        }
    }

    #[component]
    pub fn App() -> impl IntoView {
        let count = RwSignal::new(30_usize);

        // Palette for the colored rectangles.
        let palette = [
            Color::rgb(0.91, 0.30, 0.24),
            Color::rgb(0.20, 0.60, 0.86),
            Color::rgb(0.95, 0.61, 0.07),
            Color::rgb(0.18, 0.80, 0.44),
            Color::rgb(0.61, 0.35, 0.71),
            Color::rgb(0.20, 0.74, 0.74),
        ];

        view! {
            <vstack padding=12.0 gap=12.0 flex_grow=1.0>

                // ---- Vertical (existing test) ------------------------
                <label>"Vertical scroll"</label>
                <hstack gap=8.0>
                    <button on:click=move |_| count.update(|n| *n += 5)>"Add 5"</button>
                    <button on:click=move |_| count.update(|n| *n = n.saturating_sub(5))>"Remove 5"</button>
                    <label>{move || format!("{} rows", count.get())}</label>
                </hstack>
                <scroll_view flex_grow=1.0>
                    <vstack gap=2.0>
                        <For
                            each=move || {
                                let n = count.get();
                                (0..n).collect::<Vec<usize>>()
                            }
                            key=|i| *i
                            children=move |i| view! {
                                <label>{move || format!("Row {i}")}</label>
                            }
                        />
                    </vstack>
                </scroll_view>

                // ---- Horizontal -------------------------------------
                <label>"Horizontal scroll"</label>
                <scroll_view
                    axis=ScrollAxis::Horizontal
                    min_height=100.0
                    autohides_scrollers=true
                >
                    <hstack gap=8.0>
                        {(0..12).map(|i| {
                            let c = palette[i % palette.len()];
                            rect(c, 120.0, 80.0, format!("H{i}"))
                        }).collect::<Vec<_>>()}
                    </hstack>
                </scroll_view>

                // ---- Both -------------------------------------------
                // A 8x8 grid of 50px squares = 400x400 content inside
                // a 240x160 viewport, so both axes overflow.
                <label>"Both axes scroll"</label>
                <scroll_view
                    axis=ScrollAxis::Both
                    min_width=240.0
                    max_width=240.0
                    min_height=160.0
                    max_height=160.0
                    autohides_scrollers=true
                >
                    <vstack gap=4.0>
                        {(0..8).map(|row| view! {
                            <hstack gap=4.0>
                                {(0..8).map(|col| {
                                    let c = palette[(row + col) % palette.len()];
                                    rect(c, 50.0, 50.0, format!("{row},{col}"))
                                }).collect::<Vec<_>>()}
                            </hstack>
                        }).collect::<Vec<_>>()}
                    </vstack>
                </scroll_view>

            </vstack>
        }
    }

    pub fn main() {
        mount_to_window("Scroll views", (400.0, 700.0), || {
            view! { <App /> }
        }).run();
    }
}

#[cfg(target_os = "macos")]
fn main() { app::main() }

#[cfg(not(target_os = "macos"))]
fn main() {}
